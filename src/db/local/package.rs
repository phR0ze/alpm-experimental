use std::path::{PathBuf, Path};
use std::fs;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::time::SystemTime;
use std::collections::HashMap;

use failure::ResultExt;
use libflate::gzip::Decoder;
use mtree::{self, MTree, Entry};

use alpm_desc::de;
use error::{Error, ErrorKind};

#[derive(Debug)]
pub struct Package {
    pub path: PathBuf,
    desc: PackageDesc,
    files: Vec<Entry>,
}

impl Package {
    pub fn from_local(path: PathBuf, name: &str, version: &str)
        -> Result<Package, Error>
    {
        // get package description
        let desc_raw = fs::read_to_string(path.join("desc"))?;
        let desc: PackageDesc = de::from_str(&desc_raw)
            .context(ErrorKind::InvalidLocalPackage(name.to_owned()))?;

        // check package name/version with path
        if desc.name != name {
            return Err(format_err!(r#"Name on system ("{}") does not match name in package ("{}")"#, name, desc.name)
            .context(ErrorKind::InvalidLocalPackage(name.to_owned())).into());
        }
        if desc.version != version {
            return Err(format_err!(r#"Version on system ("{}") does not match version in package ("{}")"#, version, desc.version)
            .context(ErrorKind::InvalidLocalPackage(name.to_owned())).into());

        }

        // get mtree
        let files = MTree::from_reader(Decoder::new(fs::File::open(path.join("mtree"))?)?)
            .collect::<Result<_, _>>()?;

        Ok(Package { path, desc, files })
    }

    /// The package name.
    pub fn name(&self) -> &str {
        &self.desc.name
    }

    /// The package version.
    pub fn version(&self) -> &str {
        &self.desc.version
    }

    /// The base of this package.
    pub fn base(&self) -> Option<&str> {
        self.desc.base.as_ref().map(|v| v.as_ref())
    }

    /// The package description.
    pub fn description(&self) -> &str {
        &self.desc.description
    }

    /// The groups this package is in.
    pub fn groups(&self) -> impl Iterator<Item=&str> {
        self.desc.groups.iter().map(|v| v.as_ref())
    }

    /// The url for this package.
    pub fn url(&self) -> &str {
        &self.desc.url
    }

    /// The license for this package, if any.
    pub fn license(&self) -> Option<&str> {
        self.desc.license.as_ref().map(|v| v.as_ref())
    }

    /// The computer architecture this package is compiled for.
    pub fn arch(&self) -> &str {
        &self.desc.arch
    }

    /// The person who created this package
    pub fn packager(&self) -> &str {
        &self.desc.packager
    }

    /// The reason this package was installed, if given.
    pub fn reason(&self) -> Option<Reason> {
        self.desc.reason
    }

    /// The available types of validation for this package.
    pub fn validation(&self) -> &[Validation] {
        &self.desc.validation
    }

    /// The size in bytes of this package.
    pub fn size(&self) -> u64 {
        self.desc.size
    }

    /// Which packages this package replaces.
    pub fn replaces(&self) -> impl Iterator<Item=&str> {
        self.desc.replaces.iter().map(|v| v.as_ref())
    }

    /// Which packages this package depends on.
    pub fn depends(&self) -> impl Iterator<Item=&str> {
        self.desc.depends.iter().map(|v| v.as_ref())
    }

    /// Which packages this package optionally depends on.
    pub fn optional_depends(&self) -> impl Iterator<Item=&str> {
        self.desc.optional_depends.iter().map(|v| v.as_ref())
    }

    /// Which packages this package conflicts with.
    pub fn conflicts(&self) -> impl Iterator<Item=&str> {
        self.desc.conflicts.iter().map(|v| v.as_ref())
    }

    /// Which virtual packages this package provides.
    pub fn provides(&self) -> impl Iterator<Item=&str> {
        self.desc.provides.iter().map(|v| v.as_ref())
    }

    /// An iterator over the paths of all files in this package.
    pub fn file_names(&self) -> impl Iterator<Item=&Path> {
        self.files().map(|v| v.path())
    }

    /// An iterator over metadata for all files in this package.
    pub fn files(&self) -> impl Iterator<Item=&Entry> {
        self.files.iter()
    }

    /// The total amount of disk space that this package will take up
    pub fn total_size(&self) -> Result<u64, io::Error> {
        let mut acc = 0;
        for file in self.files() {
            // some files should be skipped as they are not installed
            if file.path() == Path::new("./.PKGINFO")
                || file.path() == Path::new("./.BUILDINFO")
                || file.path() == Path::new("./.INSTALL")
                // also skip directories, links etc.
                || ! (file.file_type() == Some(mtree::FileType::File))
            {
                continue;
            }
            match file.size() {
                Some(val) => { acc += val },
                None => { warn!("file {} does not have a size in its mtree metadata",
                                file.path().display()); }
            }
        }
        Ok(acc)
    }

    /// Make sure a package matches its metadata.
    ///
    /// There a few different sources of truth for a package. This method (aspires to) make sure
    /// they are all consistent.
    pub fn validate(&self) -> io::Result<Vec<ValidationError>> {
        info!("validating package {}", self.name());
        let mut errors = Vec::new();
        for file in self.files() {
            let path = file.path();
            // Check
            let md = match path.metadata() {
                Ok(md) => md,
                Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                    errors.push(ValidationError::FileNotFound(format!("{}", path.display())));
                    continue
                },
                Err(e) => return Err(e)
            };
            // Check file type
            if let Some(ty) = file.file_type() {
                match (FileType::from(ty), FileType::from(md.file_type())) {
                    (FileType::File, FileType::File) => (),
                    (FileType::File, ty) => {
                        errors.push(ValidationError::wrong_type(FileType::File, ty));
                    },
                    (FileType::Directory, FileType::Directory) => (),
                    (FileType::Directory, ty) => {
                        errors.push(ValidationError::wrong_type(FileType::Directory, ty));
                    },
                    (FileType::SymbolicLink, FileType::SymbolicLink) => (),
                    (FileType::SymbolicLink, ty) => {
                        errors.push(ValidationError::wrong_type(FileType::SymbolicLink, ty));
                    },
                    _ => ()
                }
            }
            // Check size
            if let Some(size) = file.size() {
                if md.len() != size {
                    errors.push(ValidationError::WrongSize {
                        expected: size,
                        actual: md.len()
                    });
                }
            }
        }
        Ok(errors)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct PackageDesc {
    name: String,
    version: String,
    base: Option<String>,
    #[serde(rename = "desc")]
    description: String,
    #[serde(default)]
    groups: Vec<String>,
    url: String,
    license: Option<String>,
    arch: String,
    //build_date: SystemTime,
    //install_date: SystemTime,
    packager: String,
    reason: Option<Reason>,
    validation: Vec<Validation>,
    size: u64,
    #[serde(default)]
    replaces: Vec<String>,
    #[serde(default)]
    depends: Vec<String>,
    #[serde(rename = "optdepends")]
    #[serde(default)]
    optional_depends: Vec<String>,
    #[serde(default)]
    conflicts: Vec<String>,
    #[serde(default)]
    provides: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Files {
    #[serde(default)]
    files: Vec<PathBuf>
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Validation {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "md5")]
    Md5,
    #[serde(rename = "sha256")]
    Sha256,
    #[serde(rename = "pgp")]
    Pgp,
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub enum Reason {
    /// This package was explicitally installed
    #[serde(rename = "0")]
    Explicit,
    /// This package was installed because it was required for another package
    #[serde(rename = "1")]
    Depend,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FileType {
    File,
    Directory,
    SymbolicLink,
    Other,
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FileType::File => f.write_str("file"),
            FileType::Directory => f.write_str("directory"),
            FileType::SymbolicLink => f.write_str("symbolic link"),
            FileType::Other => f.write_str("other"),
        }
    }
}

impl From<mtree::FileType> for FileType {
    fn from(f: mtree::FileType) -> Self {
        match f {
            mtree::FileType::File => FileType::File,
            mtree::FileType::Directory => FileType::Directory,
            mtree::FileType::SymbolicLink => FileType::SymbolicLink,
            _ => FileType::Other,
        }
    }
}

impl From<fs::FileType> for FileType {
    fn from(f: fs::FileType) -> FileType {
        if f.is_file() {
            FileType::File
        } else if f.is_dir() {
            FileType::Directory
        } else if f.is_symlink() {
            FileType::SymbolicLink
        } else {
            FileType::Other
        }
    }
}

/// Possible problems with a package.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Fail)]
pub enum ValidationError {
    /// A file in the package is not present on disk.
    #[fail(display = "file missing at \"{}\"", _0)]
    FileNotFound(String),
    /// A file is the wrong type
    #[fail(display = "database says file should be a {}, found a {}", expected, actual)]
    WrongType {
        expected: FileType,
        actual: FileType,
    },
    /// A file is the wrong size
    #[fail(display = "database says file should be {} bytes, found {}", expected, actual)]
    WrongSize {
        expected: u64,
        actual: u64,
    },
}

impl ValidationError {
    /// Constructor for FileNotFound variant
    fn file_not_found(s: impl Into<String>) -> ValidationError {
        ValidationError::FileNotFound(s.into())
    }
    /// Constructor for WrongType variant
    fn wrong_type(expected: impl Into<FileType>, actual: impl Into<FileType>) -> ValidationError {
        ValidationError::WrongType {
            expected: expected.into(),
            actual: actual.into()
        }
    }
    /// Constructor for WrongSize variant
    fn wrong_size(expected: impl Into<u64>, actual: impl Into<u64>) -> ValidationError {
        ValidationError::WrongSize {
            expected: expected.into(),
            actual: actual.into()
        }
    }
}
