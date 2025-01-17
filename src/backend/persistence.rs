use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use log::{error, info};
use serde::{Serialize, Deserialize};

pub struct ArticleRepository {
    path: PathBuf,
    lock: RwLock<()>
}

impl ArticleRepository {
    fn create_default_file_if_absent(path: impl AsRef<Path>) {
        if !path.as_ref().exists() {
            let mut file = File::options().write(true).read(true).create(true).open(path.as_ref()).unwrap();
            write!(
                &mut (file),
                "{default_json}",
                default_json = serde_json::to_string(&FileScheme::empty()).unwrap()
            ).unwrap();
        }
    }

    // TODO: 誤って同じパスに対してこのメソッドを二回以上呼ぶと破滅する
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self::create_default_file_if_absent(path.as_ref());

        Self {
            path: path.as_ref().to_path_buf(),
            lock: RwLock::new(())
        }
    }

    fn get_write_handle(&self) -> (Result<File>, RwLockWriteGuard<'_, ()>) {
        (File::options().write(true).open(&self.path).context("open file"), self.lock.write().unwrap())
    }

    fn get_read_handle(&self) -> (Result<File>, RwLockReadGuard<'_, ()>) {
        (File::options().read(true).open(&self.path).context("open file"), self.lock.read().unwrap())
    }

    pub async fn set_entry(&self, article_id: ArticleId, article_content: String) -> Result<()> {
        info!("calling add_entry");
        let mut a = self.parse_file_as_json()?;
        info!("parsed");
        let (file, _lock) = self.get_write_handle();
        let file = file?;

        {
            (&mut a.data).insert(article_id.clone(), Article {
                created_at: Local::now(),
                // visible: false,
                content: article_content,
                id: article_id,
            });
            info!("modified");
        }

        serde_json::to_writer(file, &a)?;
        info!("wrote");
        Ok(())
    }

    pub async fn read_snapshot(&self, article_id: &ArticleId) -> Result<Article> {
        info!("calling read");
        let a = self.parse_file_as_json()?;
        a.data.get(article_id).cloned().context(format!("read_snapshot: failed to get {article_id:?}"))
    }

    pub async fn exists(&self, article_id: &ArticleId) -> Result<bool> {
        info!("calling exists");
        let a = self.parse_file_as_json()?;
        Ok(a.data.contains_key(article_id))
    }

    pub async fn remove(&self, article_id: &ArticleId) -> Result<()> {
        info!("calling remove");
        let mut a = self.parse_file_as_json()?;
        info!("parsed");
        let (file, _lock) = self.get_write_handle();
        let file = file?;

        {
            (&mut a.data).remove(article_id);
            info!("modified");
        }

        let json = serde_json::to_string(&a)?;
        write!(
            &mut BufWriter::new(&file),
            "{json}"
        )?;

        // You must truncate, or you will be fired
        file.set_len(json.len() as u64)?;

        info!("wrote");
        Ok(())
    }

    pub(in crate::backend) fn parse_file_as_json(&self) -> Result<FileScheme> {
        let (file, _lock) = self.get_read_handle();
        let mut read_all = BufReader::new(file?);
        let mut buf = vec![];
        read_all.read_to_end(&mut buf).context("verify file")?;
        let got = String::from_utf8(buf).context("utf8 verify")?;
        info!("file JSON: {got}", got = &got);

        serde_json::from_str(got.as_str()).map_err(|e| {
            error!("{e}", e = &e);
            e
        }).context("reading json file")
    }
}

#[derive(Serialize, Deserialize)]
pub(in crate::backend) struct FileScheme {
    // TODO: この形式で永続化されるのは好みではないが、実装の速度を優先して形式の調整は凍結する
    pub(in crate::backend) data: HashMap<ArticleId, Article>
}

impl FileScheme {
    fn empty() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Article {
    pub created_at: DateTime<Local>,
    pub content: String,
    pub id: ArticleId
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct ArticleId(String);

impl ArticleId {
    pub const fn new(s: String) -> Self {
        Self(s)
    }
}

impl Display for ArticleId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}