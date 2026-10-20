//! Server-owned per-tunnel forwarding. No historical traffic accounting.
use anyhow::Result;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::Mutex,
    task::{JoinHandle, JoinSet},
    time::{sleep, Instant},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub id: String,
    pub bind: SocketAddr,
    pub backend: SocketAddr,
    pub bps: Option<u64>,
    pub udp: bool,
}
struct Running {
    spec: Spec,
    task: JoinHandle<()>,
    lease: Arc<AtomicU64>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}
#[derive(Default)]
pub struct Gateway {
    entries: HashMap<String, Running>,
}
impl Drop for Gateway {
    fn drop(&mut self) {
        for r in self.entries.values() {
            r.task.abort();
        }
    }
}
impl Gateway {
    // Stop all removed/changed limits before starting replacements. This also
    // prevents old high limits coexisting with new high limits on reallocations.
    pub async fn apply(&mut self, specs: Vec<Spec>) -> Result<()> {
        let removed: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, r)| !specs.contains(&r.spec) || r.task.is_finished())
            .map(|(id, _)| id.clone())
            .collect();
        for id in removed {
            if let Some(mut r) = self.entries.remove(&id) {
                if let Some(stop) = r.stop.take() {
                    let _ = stop.send(());
                }
                let _ = r.task.await;
            }
        }
        for spec in specs {
            if self.entries.contains_key(&spec.id) {
                continue;
            }
            anyhow::ensure!(spec.bps != Some(0), "zero bandwidth cannot listen");
            let lease = Arc::new(AtomicU64::new(lease_clock() + 15));
            let bucket = Arc::new(Limiter::new(spec.bps));
            let (stop, stopped) = tokio::sync::oneshot::channel();
            // macOS permits a wildcard listener beside an existing loopback listener.
            // Probe loopback explicitly so local Web traffic cannot hit another app.
            if spec.bind.ip().is_unspecified() {
                let local = SocketAddr::from(([127, 0, 0, 1], spec.bind.port()));
                if spec.udp {
                    drop(std::net::UdpSocket::bind(local)?);
                } else {
                    let probe = tokio::net::TcpSocket::new_v4()?;
                    probe.set_reuseaddr(true)?;
                    probe.bind(local)?;
                }
            }
            let task = if spec.udp {
                let socket = Arc::new(UdpSocket::bind(spec.bind).await?);
                tokio::spawn(udp(socket, spec.backend, bucket, lease.clone(), stopped))
            } else {
                let listener = TcpListener::bind(spec.bind).await?;
                tokio::spawn(tcp(listener, spec.backend, bucket, lease.clone(), stopped))
            };
            self.entries.insert(
                spec.id.clone(),
                Running {
                    spec,
                    task,
                    lease,
                    stop: Some(stop),
                },
            );
        }
        self.renew();
        Ok(())
    }
    pub fn renew(&self) {
        for r in self.entries.values() {
            r.lease.store(lease_clock() + 15, Ordering::Relaxed);
        }
    }
}
struct Limiter {
    bytes_per_second: Option<f64>,
    capacity: usize,
    next: Mutex<Instant>,
}
impl Limiter {
    fn new(bps: Option<u64>) -> Self {
        Self {
            bytes_per_second: bps.map(|b| b as f64 / 8.),
            capacity: bps.map_or(65507, |b| ((b / 8).clamp(1, 65507)) as usize),
            next: Mutex::new(Instant::now()),
        }
    }
    async fn acquire(&self, n: usize) {
        let Some(rate) = self.bytes_per_second else {
            return;
        };
        // FIFO async mutex provides bounded per-flow fairness in both directions.
        let mut next = self.next.lock().await;
        let deadline = (*next).max(Instant::now()) + Duration::from_secs_f64(n as f64 / rate);
        sleep(deadline.saturating_duration_since(Instant::now())).await;
        *next = deadline;
    }
}
fn lease_clock() -> u64 {
    static START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
    START.elapsed().as_secs()
}
fn alive(lease: &AtomicU64) -> bool {
    (lease_clock()) < lease.load(Ordering::Relaxed)
}
async fn tcp(
    listener: TcpListener,
    backend: SocketAddr,
    bucket: Arc<Limiter>,
    lease: Arc<AtomicU64>,
    mut stopped: tokio::sync::oneshot::Receiver<()>,
) {
    let mut jobs = JoinSet::new();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _=&mut stopped=>break,
            _=tick.tick()=>{if !alive(&lease){break;}}
            Some(_)=jobs.join_next(), if !jobs.is_empty()=>{}
            accepted=listener.accept()=>{let Ok((front,_))=accepted else{break};if jobs.len()>=256{continue;} let bucket=bucket.clone();jobs.spawn(async move{
                let back=match tokio::time::timeout(Duration::from_secs(5),TcpStream::connect(backend)).await {Ok(Ok(back))=>back,other=>{tracing::debug!(?backend,error=?other,"gateway backend unavailable");return;}};
                let (fr,fw)=front.into_split();let(br,bw)=back.into_split();
                let _=tokio::try_join!(pump(fr,bw,bucket.clone()),pump(br,fw,bucket));
            });}
        }
    }
    jobs.shutdown().await;
}
async fn pump(
    mut r: tokio::net::tcp::OwnedReadHalf,
    mut w: tokio::net::tcp::OwnedWriteHalf,
    b: Arc<Limiter>,
) -> std::io::Result<()> {
    let mut buf = vec![0; 4096.min(b.capacity)];
    loop {
        let n = r.read(&mut buf).await?;
        if n == 0 {
            w.shutdown().await?;
            return Ok(());
        }
        b.acquire(n).await;
        w.write_all(&buf[..n]).await?;
    }
}
async fn udp(
    front: Arc<UdpSocket>,
    backend: SocketAddr,
    bucket: Arc<Limiter>,
    lease: Arc<AtomicU64>,
    mut stopped: tokio::sync::oneshot::Receiver<()>,
) {
    let mut peers: HashMap<SocketAddr, tokio::sync::mpsc::Sender<Vec<u8>>> = HashMap::new();
    let mut jobs = JoinSet::new();
    let mut buf = vec![0; 65536];
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _=&mut stopped=>break,
            _=tick.tick()=>{if !alive(&lease){break;}peers.retain(|_,tx|!tx.is_closed());}
            Some(_)=jobs.join_next(), if !jobs.is_empty()=>{}
            packet=front.recv_from(&mut buf)=>{let Ok((n,peer))=packet else{break};if n>bucket.capacity{tracing::debug!(size=n,max=bucket.capacity,"UDP packet exceeds bandwidth queue capacity; dropped");continue;}
                if let Some(tx)=peers.get(&peer){let _=tx.try_send(buf[..n].to_vec());continue;}
                if peers.len()>=256{continue;}
                let Ok(back)=UdpSocket::bind("127.0.0.1:0").await else{continue};if back.connect(backend).await.is_err(){continue;}
                let(tx,mut rx)=tokio::sync::mpsc::channel::<Vec<u8>>(8);let _=tx.try_send(buf[..n].to_vec());peers.insert(peer,tx);
                let b=bucket.clone();let f=front.clone();jobs.spawn(async move{
                    let mut response=vec![0;65536];loop{tokio::select!{
                        _=sleep(Duration::from_secs(30))=>break,
                        item=rx.recv()=>{let Some(data)=item else{break};b.acquire(data.len()).await;if back.send(&data).await.is_err(){break;}},
                        result=back.recv(&mut response)=>{let Ok(n)=result else{break};if n>b.capacity{continue;}b.acquire(n).await;let _=f.send_to(&response[..n],peer).await;}
                    }}
                });
            }
        }
    }
    jobs.shutdown().await;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn unlimited_has_no_rate_wait_and_allows_full_udp_payload() {
        let limiter = Limiter::new(None);
        assert_eq!(limiter.capacity, 65507);
        tokio::time::timeout(Duration::from_millis(100), limiter.acquire(100_000_000))
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn shared_bidirectional_budget_and_cancellation() {
        let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let backend = echo.local_addr().unwrap();
        let echo_task = tokio::spawn(async move {
            loop {
                let (mut s, _) = echo.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut b = [0; 4096];
                    while let Ok(n) = s.read(&mut b).await {
                        if n == 0 {
                            break;
                        }
                        if s.write_all(&b[..n]).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bind = probe.local_addr().unwrap();
        drop(probe);
        let mut gateway = Gateway::default();
        gateway
            .apply(vec![Spec {
                id: "test".into(),
                bind,
                backend,
                bps: Some(64000),
                udp: false,
            }])
            .await
            .unwrap();
        let start = Instant::now();
        let mut jobs = JoinSet::new();
        for _ in 0..2 {
            jobs.spawn(async move {
                let mut c = TcpStream::connect(bind).await.unwrap();
                c.write_all(&[5; 4000]).await.unwrap();
                let mut b = [0; 4000];
                c.read_exact(&mut b).await.unwrap();
                assert_eq!(b, [5; 4000]);
            });
        }
        while let Some(r) = jobs.join_next().await {
            r.unwrap();
        }
        assert!(start.elapsed() >= Duration::from_millis(1900));
        assert!(start.elapsed() < Duration::from_secs(5));
        gateway.apply(vec![]).await.unwrap();
        assert!(TcpStream::connect(bind).await.is_err());
        echo_task.abort();
    }
    #[tokio::test]
    async fn udp_shared_rate_independent_tunnels_and_lease() {
        let echo = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let backend = echo.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut buf = [0; 65536];
            loop {
                let (n, p) = echo.recv_from(&mut buf).await.unwrap();
                echo.send_to(&buf[..n], p).await.unwrap();
            }
        });
        let mut specs = vec![];
        for id in ["a", "b"] {
            let probe = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            specs.push(Spec {
                id: id.into(),
                bind: probe.local_addr().unwrap(),
                backend,
                bps: Some(64000),
                udp: true,
            });
        }
        let mut g = Gateway::default();
        g.apply(specs.clone()).await.unwrap();
        let start = Instant::now();
        let mut peers = JoinSet::new();
        for spec in &specs {
            for _ in 0..2 {
                let bind = spec.bind;
                peers.spawn(async move {
                    let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
                    peer.send_to(&[7; 2000], bind).await.unwrap();
                    let mut buf = [0; 2000];
                    let n = tokio::time::timeout(Duration::from_secs(4), peer.recv(&mut buf))
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(n, 2000);
                    assert_eq!(buf, [7; 2000]);
                });
            }
        }
        while let Some(r) = peers.join_next().await {
            r.unwrap();
        }
        assert!(start.elapsed() >= Duration::from_millis(950));
        assert!(
            start.elapsed() < Duration::from_millis(1900),
            "independent tunnels must not share one bucket"
        );
        for _ in 0..10 {
            specs[0].bps = specs[0].bps.map(|b| b + 8);
            g.apply(specs.clone()).await.unwrap();
            let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            peer.send_to(b"x", specs[0].bind).await.unwrap();
            let mut buf = [0];
            tokio::time::timeout(Duration::from_secs(1), peer.recv(&mut buf))
                .await
                .unwrap()
                .unwrap();
        }
        // Lease loss closes live sockets and forgets UDP peer queues.
        for r in g.entries.values() {
            r.lease.store(0, Ordering::Relaxed);
        }
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(g.entries.values().all(|r| r.task.is_finished()));
        for spec in specs {
            assert!(UdpSocket::bind(spec.bind).await.is_ok());
        }
        task.abort();
    }
}
