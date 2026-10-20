use crate::model::*;
use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
pub struct Store {
    pub db: Connection,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path)?;
        let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        anyhow::ensure!(
            version <= 4,
            "Database schema {version} is newer than supported schema 4; refusing to downgrade"
        );
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS clients(id TEXT PRIMARY KEY,name TEXT NOT NULL,token_hash TEXT UNIQUE NOT NULL,active INTEGER NOT NULL DEFAULT 1,last_seen INTEGER NOT NULL DEFAULT 0,running INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS tunnels(id TEXT PRIMARY KEY,name TEXT NOT NULL,client_id TEXT NOT NULL REFERENCES clients(id),protocol TEXT NOT NULL,local_host TEXT NOT NULL,local_port INTEGER NOT NULL,public_port INTEGER UNIQUE NOT NULL,domain TEXT UNIQUE,enabled INTEGER NOT NULL,service_token TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS audit(id INTEGER PRIMARY KEY,created_at INTEGER NOT NULL,action TEXT NOT NULL,target TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sites(id TEXT PRIMARY KEY,name TEXT NOT NULL,domain TEXT UNIQUE NOT NULL,upstream_host TEXT NOT NULL,upstream_port INTEGER NOT NULL,enabled INTEGER NOT NULL);
COMMIT;")?;
        let store = Self { db };
        crate::tenant::migrate(&store)?;
        Ok(store)
    }
    /// Atomic desired-state mutation, revision and secret-free audit record.
    pub fn change<T>(
        &self,
        action: &str,
        target: &str,
        change: impl FnOnce(&Self) -> Result<T>,
    ) -> Result<T> {
        let tx = self.db.unchecked_transaction()?;
        let result = change(self)?;
        let revision = self
            .get("revision")?
            .unwrap_or_default()
            .parse::<u64>()
            .unwrap_or(0)
            + 1;
        self.set("revision", &revision.to_string())?;
        self.db.execute(
            "INSERT INTO audit(created_at,action,target) VALUES(?,?,?)",
            params![now(), action, target],
        )?;
        self.db.execute(
            "DELETE FROM audit WHERE id <= (SELECT COALESCE(MAX(id),0)-1000 FROM audit)",
            [],
        )?;
        tx.commit()?;
        Ok(result)
    }
    pub fn audit(&self) -> Result<Vec<serde_json::Value>> {
        let mut q = self
            .db
            .prepare("SELECT id,created_at,action,target FROM audit ORDER BY id DESC LIMIT 100")?;
        let rows=q.query_map([],|r|Ok(serde_json::json!({"id":r.get::<_,i64>(0)?,"created_at":r.get::<_,i64>(1)?,"action":r.get::<_,String>(2)?,"target":r.get::<_,String>(3)?})))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    pub fn clients(&self) -> Result<Vec<Client>> {
        let mut s = self
            .db
            .prepare("SELECT id,name,active,last_seen,running FROM clients ORDER BY rowid")?;
        let rows = s.query_map([], |r| {
            Ok(Client {
                id: r.get(0)?,
                name: r.get(1)?,
                active: r.get(2)?,
                last_seen: r.get(3)?,
                running: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    pub fn tunnels(&self) -> Result<Vec<Tunnel>> {
        let mut s=self.db.prepare("SELECT id,name,client_id,protocol,local_host,local_port,public_port,domain,enabled,service_token FROM tunnels ORDER BY rowid")?;
        let rows = s.query_map([], |r| {
            Ok(Tunnel {
                id: r.get(0)?,
                name: r.get(1)?,
                client_id: r.get(2)?,
                protocol: r.get(3)?,
                local_host: r.get(4)?,
                local_port: r.get(5)?,
                public_port: r.get(6)?,
                domain: r.get(7)?,
                enabled: r.get(8)?,
                service_token: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    pub fn sites(&self) -> Result<Vec<Site>> {
        let mut q = self.db.prepare(
            "SELECT id,name,domain,upstream_host,upstream_port,enabled FROM sites ORDER BY rowid",
        )?;
        let rows = q.query_map([], |r| {
            Ok(Site {
                id: r.get(0)?,
                name: r.get(1)?,
                domain: r.get(2)?,
                upstream_host: r.get(3)?,
                upstream_port: r.get(4)?,
                enabled: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    pub fn save_site(&self, site: &Site) -> Result<()> {
        self.db.execute("INSERT INTO sites VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET name=excluded.name,domain=excluded.domain,upstream_host=excluded.upstream_host,upstream_port=excluded.upstream_port,enabled=excluded.enabled",params![site.id,site.name,site.domain,site.upstream_host,site.upstream_port,site.enabled])?;
        Ok(())
    }
    pub fn active_tunnels(&self) -> Result<Vec<Tunnel>> {
        let clients = self.clients()?;
        Ok(self
            .tunnels()?
            .into_iter()
            .filter(|t| {
                t.enabled
                    && clients.iter().any(|c| c.id == t.client_id && c.active)
                    && crate::tenant::owner_active(self, &t.client_id)
                    && crate::tenant::resource_allowed(self, t)
            })
            .collect())
    }
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .db
            .query_row("SELECT value FROM settings WHERE key=?", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.db.execute("INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value WHERE settings.value<>excluded.value",params![key,value])?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings> {
        Ok(serde_json::from_str(
            &self
                .get("config")?
                .ok_or_else(|| anyhow::anyhow!("settings missing"))?,
        )?)
    }
    pub fn save_tunnel(&self, t: &Tunnel) -> Result<()> {
        self.db.execute("INSERT INTO tunnels VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(id) DO UPDATE SET name=excluded.name,client_id=excluded.client_id,protocol=excluded.protocol,local_host=excluded.local_host,local_port=excluded.local_port,public_port=excluded.public_port,domain=excluded.domain,enabled=excluded.enabled,service_token=excluded.service_token",params![t.id,t.name,t.client_id,t.protocol,t.local_host,t.local_port,t.public_port,t.domain,t.enabled,t.service_token])?;
        Ok(())
    }
}
