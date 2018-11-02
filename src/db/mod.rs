//! Functionality relating to alpm databases (local and sync).
use std::borrow::Cow;
use std::cell::{Ref, RefCell};
use std::cmp;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fmt::{self, Display};
use std::fs;
use std::io::{self, Read, Write};
use std::ops::Deref;
use std::path::{self, Path, PathBuf};
use std::rc::{Rc, Weak as WeakRc};

use atoi::atoi;
use error::{Error, ErrorKind};
use failure::{err_msg, Fail, ResultExt};
use fs2::FileExt;
use Handle;

mod local;
mod sync;

pub(crate) use self::local::LocalDatabaseInner;
pub use self::local::{LocalDatabase, LocalDbPackage};
pub use self::sync::SyncDatabase;
pub(crate) use self::sync::{SyncDatabaseInner, SyncDbName};

/// The name of the directory for sync databases.
pub const SYNC_DB_DIR: &str = "sync";
/// The extension of the directory for sync databases.
pub const DEFAULT_SYNC_DB_EXT: &str = "db";
/// The name of the local database.
pub const LOCAL_DB_NAME: &str = "local";

/*
pub trait Database {
    type Pkg: Package;
    type PkgIter: Iterator<Item=Self::Pkg>;
    type Path: Deref<Target=Path>;

    /// Get the name of this database
    fn name(&self) -> &str;

    /// Get the path of the root file or directory for this database.
    fn path(&self) -> Self::Path;

    /// Get the status of this database.
    fn status(&self) -> Result<DbStatus, Error>;

    /// Synchronize the database with any external sources.
    fn synchronize(&self, force: bool) -> Result<(), Error> {
        // do nothing by default
        Ok(())
    }

    /// Get a package in this database
    fn package(&self, name: &str) -> Self::Pkg;

    /// Get all packages in this database
    fn packages(&self) -> Self::PkgIter {
        unimplemented!()
    }
}
*/

pub trait Package {
    fn name(&self) -> &str;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum DbStatus {
    /// The database directory is not present.
    Missing,
    /// The database directory is present.
    Exists {
        /// Whether the database is consistent.
        valid: bool,
    },
}
/*
bitflags! {
    pub struct DbStatus: u32 {
        const VALID         = 0x0000_0001;
        const INVALID       = 0x0000_0002;
        const EXISTS        = 0x0000_0004;
        const MISSING       = 0x0000_0008;
        const LOCAL         = 0x0000_0400;
        const PACKAGE_CACHE = 0x0000_0800;
        const GROUP_CACHE   = 0x0000_1000;
    }
}
*/

bitflags! {
    /// What this database is to be used for.
    pub struct DbUsage: u32 {
        const SYNC    = 0b0001;
        const SEARCH  = 0b0010;
        const INSTALL = 0b0100;
        const UPGRADE = 0b1000;
        const ALL     = Self::SYNC.bits |
                        Self::SEARCH.bits |
                        Self::INSTALL.bits |
                        Self::UPGRADE.bits;
    }
}

impl Default for DbUsage {
    fn default() -> Self {
        DbUsage::ALL
    }
}

/// The trust level that signatures must match.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SignatureLevel {
    /// Inherit the signature level required from the parent Alpm instance.
    Inherit,
    Optional,
    MarginalOk,
    UnknownOk,
}

impl Default for SignatureLevel {
    fn default() -> Self {
        SignatureLevel::Inherit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test(ignore)]
    fn db_path() {
        let base_path = "/var/lib/pacman/";
        let base_path2 = "/var/lib/pacman";
        let ext = "db";

        let tests = vec![("sync1", "/var/lib/pacman/sync/sync1.db")];
        for (db_name, target) in tests {
            let db_name = SyncDbName::new(db_name).unwrap();
            let target = Path::new(target);
            assert_eq!(db_name.path(&base_path, &ext), target);
            assert_eq!(db_name.path(&base_path2, &ext), target);
        }
    }
}
