use cfg_if::cfg_if;
use chrono::{DateTime, Utc};
use getset::{CopyGetters, Getters};
#[cfg(unix)]
use users::{get_current_gid, get_current_uid};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

cfg_if! {
    if #[cfg(unix)] {
        // File type bits
        const S_IFMT: u32 = 0o170000; // bit mask for the file type bit field
        const S_IFREG: u32 = 0o100000; // regular file
        const S_IFDIR: u32 = 0o040000; // directory
        const S_IFLNK: u32 = 0o120000; // symbolic link

        // Permission bits
        const S_IRWXU: u32 = 0o700; // user (file owner) has read, write, and execute permission
        const S_IRUSR: u32 = 0o400; // user has read permission
        const S_IWUSR: u32 = 0o200; // user has write permission
        const S_IXUSR: u32 = 0o100; // user has execute permission

        const S_IRWXG: u32 = 0o070; // group has read, write, and execute permission
        const S_IRGRP: u32 = 0o040; // group has read permission
        const S_IWGRP: u32 = 0o020; // group has write permission
        const S_IXGRP: u32 = 0o010; // group has execute permission

        const S_IRWXO: u32 = 0o007; // others have read, write, and execute permission
        const S_IROTH: u32 = 0o004; // others have read permission
        const S_IWOTH: u32 = 0o002; // others have write permission
        const S_IXOTH: u32 = 0o001; // others have execute permission

        // Permission mask
        const S_IPERM: u32 = 0o777; // mask for permission bits

        // Combined permission constants for user, group, and other
        const USER_RW: u32 = S_IRUSR | S_IWUSR;
        const USER_RX: u32 = S_IRUSR | S_IXUSR;
        const USER_WX: u32 = S_IWUSR | S_IXUSR;

        const GROUP_RW: u32 = S_IRGRP | S_IWGRP;
        const GROUP_RX: u32 = S_IRGRP | S_IXGRP;
        const GROUP_WX: u32 = S_IWGRP | S_IXGRP;

        const OTHER_RW: u32 = S_IROTH | S_IWOTH;
        const OTHER_RX: u32 = S_IROTH | S_IXOTH;
        const OTHER_WX: u32 = S_IWOTH | S_IXOTH;
    }
}

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Metadata for a file or directory in the virtual filesystem.
///
/// This struct holds standard Unix-like metadata including:
/// - File mode (type and permissions)
/// - File size
/// - Creation timestamp
/// - Last modification timestamp
/// - Last access timestamp
/// - User ID (Unix only)
/// - Group ID (Unix only)
#[derive(Debug, Clone, CopyGetters, Getters, PartialEq, Eq)]
pub struct Metadata {
    /// The mode of the file, combining file type and permissions
    #[cfg(unix)]
    #[getset(get = "pub with_prefix")]
    mode: Mode,

    #[cfg(not(unix))]
    #[getset(get = "pub with_prefix")]
    entity_type: EntityType,

    /// Size of the file in bytes
    #[getset(get_copy = "pub with_prefix")]
    size: u64,

    /// When the file was created
    #[getset(get = "pub with_prefix")]
    created_at: DateTime<Utc>,

    /// When the file was last modified
    #[getset(get = "pub with_prefix")]
    modified_at: DateTime<Utc>,

    /// When the file was last accessed
    #[getset(get = "pub with_prefix")]
    accessed_at: DateTime<Utc>,

    /// User ID of the file owner (Unix only)
    #[cfg(unix)]
    #[getset(get_copy = "pub with_prefix")]
    uid: u32,

    /// Group ID of the file group (Unix only)
    #[cfg(unix)]
    #[getset(get_copy = "pub with_prefix")]
    gid: u32,
}

cfg_if! {
    if #[cfg(unix)] {
        /// A Unix-style file mode that combines file type and permission bits.
        ///
        /// The mode is represented as a 32-bit integer with the following bit layout:
        /// ```text
        /// Bits  Description
        /// ----  -----------
        /// 15-12 File type (S_IFMT)
        ///       - Regular file     (S_IFREG)  0o100000
        ///       - Directory       (S_IFDIR)  0o040000
        ///       - Symbolic link   (S_IFLNK)  0o120000
        ///
        /// 8-6   User permissions
        ///       - Read            (S_IRUSR)  0o400
        ///       - Write           (S_IWUSR)  0o200
        ///       - Execute         (S_IXUSR)  0o100
        ///
        /// 5-3   Group permissions
        ///       - Read            (S_IRGRP)  0o040
        ///       - Write           (S_IWGRP)  0o020
        ///       - Execute         (S_IXGRP)  0o010
        ///
        /// 2-0   Other permissions
        ///       - Read            (S_IROTH)  0o004
        ///       - Write           (S_IWOTH)  0o002
        ///       - Execute         (S_IXOTH)  0o001
        /// ```
        ///
        /// This struct provides methods to get and set both the file type and permission bits,
        /// maintaining compatibility with standard Unix file mode conventions.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Mode(u32);

        /// The type of a file in the filesystem.
        #[repr(u32)]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ModeType {
            /// Regular file
            File = 0o100000,

            /// Directory
            Directory = 0o040000,

            /// Symbolic link
            Symlink = 0o120000,
        }

        /// Unix-style user permission flags (bits 8-6)
        ///
        /// Bit layout (bits 8-6):
        /// ```text
        /// 8 7 6
        /// r w x
        /// ```
        #[repr(u32)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum User {
            /// Read permission
            R = S_IRUSR,

            /// Write permission
            W = S_IWUSR,

            /// Execute permission
            X = S_IXUSR,

            /// Read + Write
            RW = S_IRUSR | S_IWUSR,

            /// Read + Execute
            RX = S_IRUSR | S_IXUSR,

            /// Write + Execute
            WX = S_IWUSR | S_IXUSR,

            /// Read + Write + Execute
            RWX = S_IRWXU,

            /// No permissions
            None = 0,
        }

        /// Unix-style group permission flags (bits 5-3)
        ///
        /// Bit layout (bits 5-3):
        /// ```text
        /// 5 4 3
        /// r w x
        /// ```
        #[repr(u32)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Group {
            /// Read permission
            R = S_IRGRP,

            /// Write permission
            W = S_IWGRP,

            /// Execute permission
            X = S_IXGRP,

            /// Read + Write
            RW = S_IRGRP | S_IWGRP,

            /// Read + Execute
            RX = S_IRGRP | S_IXGRP,

            /// Write + Execute
            WX = S_IWGRP | S_IXGRP,

            /// Read + Write + Execute
            RWX = S_IRWXG,

            /// No permissions
            None = 0,
        }

        /// Unix-style other permission flags (bits 2-0)
        ///
        /// Bit layout (bits 2-0):
        /// ```text
        /// 2 1 0
        /// r w x
        /// ```
        #[repr(u32)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Other {
            /// Read permission
            R = S_IROTH,

            /// Write permission
            W = S_IWOTH,

            /// Execute permission
            X = S_IXOTH,

            /// Read + Write
            RW = S_IROTH | S_IWOTH,

            /// Read + Execute
            RX = S_IROTH | S_IXOTH,

            /// Write + Execute
            WX = S_IWOTH | S_IXOTH,

            /// Read + Write + Execute
            RWX = S_IRWXO,

            /// No permissions
            None = 0,
        }

        /// A structured representation of Unix-style permission bits.
        ///
        /// This struct provides a type-safe way to work with Unix permission bits by breaking them
        /// into their user, group, and other components. Each component is represented by its
        /// respective enum (`User`, `Group`, `Other`) which ensures valid permission combinations.
        ///
        /// The permissions can be combined using the bitwise OR operator (`|`). For example:
        /// ```rust
        /// use virtualfs::{User, Group, Other};
        ///
        /// // Create permissions: rw-r--r-- (0o644)
        /// let perms = User::RW | Group::R | Other::R;
        ///
        /// // Create permissions: rwxr-x--- (0o750)
        /// let perms = User::RWX | Group::RX | Other::None;
        /// ```
        ///
        /// When converted to a mode, the permission bits occupy the lower 9 bits of the mode value,
        /// maintaining compatibility with the standard Unix permission layout:
        /// ```text
        /// user  group  other
        /// rwx   rwx    rwx
        /// ```
        #[derive(Debug, Clone, CopyGetters, Getters, PartialEq, Eq)]
        pub struct ModePerms {
            user: User,
            group: Group,
            other: Other,
        }
    }
    else {
        /// The type of an entity in the filesystem.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum EntityType {
            /// Regular file
            File,

            /// Directory
            Directory,

            /// Symbolic link
            Symlink,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Metadata {
    /// Creates a new Metadata instance with default values.
    ///
    /// Default values are:
    /// - mode: 0 (no permissions)
    /// - size: 0 bytes
    /// - created_at: current UTC time
    /// - modified_at: current UTC time
    /// - accessed_at: current UTC time
    /// - uid: current user's UID (Unix only)
    /// - gid: current user's GID (Unix only)
    ///
    /// ## Examples
    /// ```rust
    /// use virtualfs::{Metadata, ModeType};
    ///
    /// let metadata = Metadata::new(ModeType::File);
    /// assert_eq!(metadata.get_size(), 0);
    /// ```
    pub fn new(
        #[cfg(unix)] entity_type: ModeType,
        #[cfg(not(unix))] entity_type: EntityType,
    ) -> Self {
        let now = Utc::now();
        Self {
            #[cfg(unix)]
            mode: Mode::new(entity_type),
            #[cfg(not(unix))]
            entity_type,
            size: 0,
            created_at: now,
            modified_at: now,
            accessed_at: now,
            #[cfg(unix)]
            uid: get_current_uid(),
            #[cfg(unix)]
            gid: get_current_gid(),
        }
    }

    /// Sets the file type portion of the mode.
    ///
    /// This method:
    /// 1. Clears the existing file type bits
    /// 2. Sets the new file type bits based on the provided type
    ///
    /// The file type is stored in the high bits of the mode (bits 12-15).
    #[cfg(unix)]
    pub fn set_type(&mut self, entity_type: ModeType) {
        self.mode.set_type(entity_type);
    }

    /// Gets the file type from the mode.
    ///
    /// Returns:
    /// - `Some(ModeType)` if the file type bits represent a valid type
    /// - `None` if the file type bits don't match any known type
    #[cfg(unix)]
    pub fn get_type(&self) -> Option<ModeType> {
        self.mode.get_type()
    }

    /// Sets the permission bits of the mode.
    ///
    /// This method:
    /// 1. Clears the existing permission bits (bits 0-8)
    /// 2. Sets the new permission bits from the provided permissions
    ///
    /// ## Examples
    /// ```rust
    /// use virtualfs::{Metadata, ModeType, User, Group, Other};
    ///
    /// let mut metadata = Metadata::new(ModeType::File);
    /// metadata.set_permissions(User::RW | Group::R | Other::R);
    /// ```
    #[cfg(unix)]
    pub fn set_permissions(&mut self, permissions: impl Into<ModePerms>) {
        self.mode.set_permissions(permissions);
    }

    /// Gets the current permissions from the mode
    #[cfg(unix)]
    pub fn get_permissions(&self) -> ModePerms {
        self.mode.get_permissions()
    }

    /// Gets the file type of the file.
    #[cfg(not(unix))]
    pub fn get_entity_type(&self) -> &EntityType {
        &self.entity_type
    }

    /// Sets the file type of the file.
    ///
    /// ## Examples
    /// ```rust
    /// use virtualfs::{Metadata, ModeType};
    ///
    /// let mut metadata = Metadata::new(ModeType::File);
    /// metadata.set_entity_type(ModeType::Directory);
    /// assert_eq!(metadata.get_entity_type(), &ModeType::Directory);
    /// ```
    #[cfg(not(unix))]
    pub fn set_entity_type(&mut self, entity_type: EntityType) {
        self.entity_type = entity_type;
    }

    /// Sets the size of the file.
    ///
    /// ## Examples
    /// ```rust
    /// use virtualfs::{Metadata, ModeType};
    ///
    /// let mut metadata = Metadata::new(ModeType::File);
    /// metadata.set_size(100);
    /// assert_eq!(metadata.get_size(), 100);
    /// ```
    pub fn set_size(&mut self, size: u64) {
        self.size = size;
    }

    /// Sets the last access time of the file.
    pub fn set_accessed_at(&mut self, time: DateTime<Utc>) {
        self.accessed_at = time;
    }

    /// Sets the user ID of the file owner (Unix only).
    #[cfg(unix)]
    pub fn set_uid(&mut self, uid: u32) {
        self.uid = uid;
    }

    /// Sets the group ID of the file group (Unix only).
    #[cfg(unix)]
    pub fn set_gid(&mut self, gid: u32) {
        self.gid = gid;
    }

    #[cfg(test)]
    #[cfg(unix)]
    fn with_root_ownership(entity_type: ModeType) -> Self {
        let now = Utc::now();
        Self {
            mode: Mode::new(entity_type),
            size: 0,
            created_at: now,
            modified_at: now,
            accessed_at: now,
            uid: 0,
            gid: 0,
        }
    }
}

cfg_if! {
    if #[cfg(unix)] {
        impl Mode {
            /// Creates a new Mode with appropriate default permissions based on the file type.
            ///
            /// Default permissions are:
            /// - Regular files: 644 (rw-r--r--)
            /// - Directories: 755 (rwxr-xr-x)
            /// - Symlinks: 777 (rwxrwxrwx)
            pub fn new(entity_type: ModeType) -> Self {
                let default_perms = match entity_type {
                    ModeType::File => User::RW | Group::R | Other::R,
                    ModeType::Directory => User::RWX | Group::RX | Other::RX,
                    ModeType::Symlink => User::RWX | Group::RWX | Other::RWX,
                };
                Self((entity_type as u32) | u32::from(default_perms))
            }

            /// Gets the file type portion of the mode
            pub fn get_type(&self) -> Option<ModeType> {
                match self.0 & S_IFMT {
                    S_IFREG => Some(ModeType::File),
                    S_IFDIR => Some(ModeType::Directory),
                    S_IFLNK => Some(ModeType::Symlink),
                    _ => None,
                }
            }

            /// Sets the file type portion of the mode
            pub fn set_type(&mut self, entity_type: ModeType) {
                // Clear the file type bits
                self.0 &= !S_IFMT;
                // Set the new file type bits
                self.0 |= entity_type as u32;
            }

            /// Gets the permission portion of the mode
            pub fn get_permissions(&self) -> ModePerms {
                let mode = self.0 & S_IPERM;
                ModePerms {
                    user: match mode & S_IRWXU {
                        S_IRWXU => User::RWX,
                        USER_RW => User::RW,
                        USER_RX => User::RX,
                        USER_WX => User::WX,
                        S_IRUSR => User::R,
                        S_IWUSR => User::W,
                        S_IXUSR => User::X,
                        _ => User::None,
                    },
                    group: match mode & S_IRWXG {
                        S_IRWXG => Group::RWX,
                        GROUP_RW => Group::RW,
                        GROUP_RX => Group::RX,
                        GROUP_WX => Group::WX,
                        S_IRGRP => Group::R,
                        S_IWGRP => Group::W,
                        S_IXGRP => Group::X,
                        _ => Group::None,
                    },
                    other: match mode & S_IRWXO {
                        S_IRWXO => Other::RWX,
                        OTHER_RW => Other::RW,
                        OTHER_RX => Other::RX,
                        OTHER_WX => Other::WX,
                        S_IROTH => Other::R,
                        S_IWOTH => Other::W,
                        S_IXOTH => Other::X,
                        _ => Other::None,
                    },
                }
            }

            /// Sets the permission portion of the mode
            pub fn set_permissions(&mut self, perms: impl Into<ModePerms>) {
                let perms = perms.into();
                // Clear the permission bits
                self.0 &= !S_IPERM;
                // Set the new permission bits
                self.0 |= u32::from(perms);
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

cfg_if! {
    if #[cfg(unix)] {
        impl From<User> for ModePerms {
            fn from(user: User) -> Self {
                ModePerms {
                    user,
                    group: Group::None,
                    other: Other::None,
                }
            }
        }

        impl From<Group> for ModePerms {
            fn from(group: Group) -> Self {
                ModePerms {
                    user: User::None,
                    group,
                    other: Other::None,
                }
            }
        }

        impl From<Other> for ModePerms {
            fn from(other: Other) -> Self {
                ModePerms {
                    user: User::None,
                    group: Group::None,
                    other,
                }
            }
        }

        impl From<ModePerms> for u32 {
            fn from(permissions: ModePerms) -> Self {
                permissions.user as u32 | permissions.group as u32 | permissions.other as u32
            }
        }

        impl std::ops::BitOr<Group> for User {
            type Output = ModePerms;

            fn bitor(self, rhs: Group) -> Self::Output {
                ModePerms {
                    user: self,
                    group: rhs,
                    other: Other::None,
                }
            }
        }

        impl std::ops::BitOr<Other> for User {
            type Output = ModePerms;

            fn bitor(self, rhs: Other) -> Self::Output {
                ModePerms {
                    user: self,
                    group: Group::None,
                    other: rhs,
                }
            }
        }

        impl std::ops::BitOr<Other> for Group {
            type Output = ModePerms;

            fn bitor(self, rhs: Other) -> Self::Output {
                ModePerms {
                    user: User::None,
                    group: self,
                    other: rhs,
                }
            }
        }

        impl std::ops::BitOr<ModePerms> for User {
            type Output = ModePerms;

            fn bitor(self, rhs: ModePerms) -> Self::Output {
                ModePerms {
                    user: self,
                    group: rhs.group,
                    other: rhs.other,
                }
            }
        }

        impl std::ops::BitOr<ModePerms> for Group {
            type Output = ModePerms;

            fn bitor(self, rhs: ModePerms) -> Self::Output {
                ModePerms {
                    user: rhs.user,
                    group: self,
                    other: rhs.other,
                }
            }
        }

        impl std::ops::BitOr<ModePerms> for Other {
            type Output = ModePerms;

            fn bitor(self, rhs: ModePerms) -> Self::Output {
                ModePerms {
                    user: rhs.user,
                    group: rhs.group,
                    other: self,
                }
            }
        }

        impl std::ops::BitOr<Group> for ModePerms {
            type Output = ModePerms;

            fn bitor(self, rhs: Group) -> Self::Output {
                ModePerms {
                    user: self.user,
                    group: rhs,
                    other: self.other,
                }
            }
        }

        impl std::ops::BitOr<Other> for ModePerms {
            type Output = ModePerms;

            fn bitor(self, rhs: Other) -> Self::Output {
                ModePerms {
                    user: self.user,
                    group: self.group,
                    other: rhs,
                }
            }
        }

        impl std::ops::BitOr<User> for ModePerms {
            type Output = ModePerms;

            fn bitor(self, rhs: User) -> Self::Output {
                ModePerms {
                    user: rhs,
                    group: self.group,
                    other: self.other,
                }
            }
        }

        impl From<ModeType> for Mode {
            fn from(entity_type: ModeType) -> Self {
                Self(entity_type as u32)
            }
        }

        impl From<ModePerms> for Mode {
            fn from(perms: ModePerms) -> Self {
                Self(u32::from(perms))
            }
        }

        impl std::ops::BitAnd<u32> for Mode {
            type Output = u32;

            fn bitand(self, rhs: u32) -> Self::Output {
                self.0 & rhs
            }
        }

        impl std::ops::BitAndAssign<u32> for Mode {
            fn bitand_assign(&mut self, rhs: u32) {
                self.0 &= rhs;
            }
        }

        impl std::ops::BitOrAssign<u32> for Mode {
            fn bitor_assign(&mut self, rhs: u32) {
                self.0 |= rhs;
            }
        }

        impl From<u32> for Mode {
            fn from(mode: u32) -> Self {
                Self(mode)
            }
        }

        impl From<Mode> for u32 {
            fn from(mode: Mode) -> Self {
                mode.0
            }
        }

        impl std::fmt::Display for ModeType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    ModeType::File => write!(f, "-"),
                    ModeType::Directory => write!(f, "d"),
                    ModeType::Symlink => write!(f, "l"),
                }
            }
        }

        impl std::fmt::Display for ModePerms {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // User permissions
                write!(
                    f,
                    "{}{}{}",
                    if (self.user as u32 & S_IRUSR) != 0 {
                        "r"
                    } else {
                        "-"
                    },
                    if (self.user as u32 & S_IWUSR) != 0 {
                        "w"
                    } else {
                        "-"
                    },
                    if (self.user as u32 & S_IXUSR) != 0 {
                        "x"
                    } else {
                        "-"
                    }
                )?;

                // Group permissions
                write!(
                    f,
                    "{}{}{}",
                    if (self.group as u32 & S_IRGRP) != 0 {
                        "r"
                    } else {
                        "-"
                    },
                    if (self.group as u32 & S_IWGRP) != 0 {
                        "w"
                    } else {
                        "-"
                    },
                    if (self.group as u32 & S_IXGRP) != 0 {
                        "x"
                    } else {
                        "-"
                    }
                )?;

                // Other permissions
                write!(
                    f,
                    "{}{}{}",
                    if (self.other as u32 & S_IROTH) != 0 {
                        "r"
                    } else {
                        "-"
                    },
                    if (self.other as u32 & S_IWOTH) != 0 {
                        "w"
                    } else {
                        "-"
                    },
                    if (self.other as u32 & S_IXOTH) != 0 {
                        "x"
                    } else {
                        "-"
                    }
                )
            }
        }

        impl std::fmt::Display for Mode {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // File type
                match self.get_type() {
                    Some(entity_type) => write!(f, "{}", entity_type)?,
                    None => write!(f, "?")?,
                }

                // Permissions
                write!(f, "{}", self.get_permissions())
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_new() {
        let metadata = Metadata::new(ModeType::File);
        assert_eq!(metadata.get_mode(), &Mode::new(ModeType::File));
        assert_eq!(metadata.get_size(), 0);
    }

    #[test]
    fn test_entity_type() {
        let mut metadata = Metadata::new(ModeType::File);

        // Test setting and getting file type
        metadata.set_type(ModeType::File);
        assert!(matches!(metadata.get_type(), Some(ModeType::File)));

        metadata.set_type(ModeType::Directory);
        assert!(matches!(metadata.get_type(), Some(ModeType::Directory)));

        metadata.set_type(ModeType::Symlink);
        assert!(matches!(metadata.get_type(), Some(ModeType::Symlink)));

        // Test that file type bits are preserved when setting permissions
        metadata.set_type(ModeType::File);
        metadata.set_permissions(User::RWX | Group::RWX | Other::RWX);
        assert!(matches!(metadata.get_type(), Some(ModeType::File)));
    }

    #[test]
    fn test_basic_permissions() {
        let mut metadata = Metadata::new(ModeType::File);

        // Test individual permission types
        metadata.set_permissions(User::R);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o400);

        metadata.set_permissions(Group::W);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o020);

        metadata.set_permissions(Other::X);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o001);
    }

    #[test]
    fn test_combined_permissions() {
        let mut metadata = Metadata::new(ModeType::File);

        // Test combined permissions within same group
        metadata.set_permissions(User::RW);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o600);

        metadata.set_permissions(Group::RX);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o050);

        metadata.set_permissions(Other::WX);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o003);

        // Test all permissions in a group
        metadata.set_permissions(User::RWX);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o700);
    }

    #[test]
    fn test_permission_combinations() {
        let mut metadata = Metadata::new(ModeType::File);

        // Test combining different permission groups
        metadata.set_permissions(User::RW | Group::R);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o640);

        metadata.set_permissions(User::RWX | Group::RX | Other::R);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o754);

        // Test chaining operations
        let perms = User::RW | Group::R | Other::X;
        metadata.set_permissions(perms);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o641);
    }

    #[test]
    fn test_permission_updates() {
        // Test updating existing permissions
        let base_perms = User::RW | Group::R;
        let updated = base_perms | Other::X;

        let mut metadata = Metadata::new(ModeType::File);
        metadata.set_permissions(updated);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o641);

        // Test that updating one group doesn't affect others
        let perms = User::RW | Group::R;
        let with_other = perms | Other::X;
        let with_updated_group = with_other | Group::RWX;

        metadata.set_permissions(with_updated_group);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o671);
    }

    #[test]
    fn test_none_permissions() {
        let mut metadata = Metadata::new(ModeType::File);

        // Test that None permissions don't set any bits
        metadata.set_permissions(User::None);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0);

        // Test that None permissions clear existing bits for that group
        metadata.set_permissions(User::RWX | Group::RWX | Other::RWX);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o777);

        metadata.set_permissions(User::None | Group::RWX | Other::RWX);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o077);
    }

    #[test]
    fn test_permission_clearing() {
        let mut metadata = Metadata::new(ModeType::File);

        // Set all permissions
        metadata.set_permissions(User::RWX | Group::RWX | Other::RWX);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o777);

        // Clear permissions by setting new ones
        metadata.set_permissions(User::R | Group::R | Other::R);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o444);

        // Verify that setting new permissions clears old ones
        metadata.set_permissions(User::W);
        assert_eq!(u32::from(*metadata.get_mode()) & 0o777, 0o200);
    }

    #[test]
    fn test_mode_type_display() {
        assert_eq!(ModeType::File.to_string(), "-");
        assert_eq!(ModeType::Directory.to_string(), "d");
        assert_eq!(ModeType::Symlink.to_string(), "l");
    }

    #[test]
    fn test_mode_perms_display() {
        // Test common permission patterns
        assert_eq!((User::RW | Group::R | Other::R).to_string(), "rw-r--r--");
        assert_eq!(
            (User::RWX | Group::RX | Other::None).to_string(),
            "rwxr-x---"
        );
        assert_eq!(
            (User::RWX | Group::RWX | Other::RWX).to_string(),
            "rwxrwxrwx"
        );
        assert_eq!(
            (User::None | Group::None | Other::None).to_string(),
            "---------"
        );
    }

    #[test]
    fn test_mode_display() {
        let mut mode = Mode::new(ModeType::File);

        // Regular file with rw-r--r-- permissions
        mode.set_type(ModeType::File);
        mode.set_permissions(User::RW | Group::R | Other::R);
        assert_eq!(mode.to_string(), "-rw-r--r--");

        // Directory with rwxr-x--- permissions
        mode.set_type(ModeType::Directory);
        mode.set_permissions(User::RWX | Group::RX | Other::None);
        assert_eq!(mode.to_string(), "drwxr-x---");

        // Symlink with rwxrwxrwx permissions
        mode.set_type(ModeType::Symlink);
        mode.set_permissions(User::RWX | Group::RWX | Other::RWX);
        assert_eq!(mode.to_string(), "lrwxrwxrwx");
    }

    #[test]
    fn test_access_time() {
        let mut metadata = Metadata::new(ModeType::File);
        let original_atime = metadata.get_accessed_at().clone();

        // Sleep briefly to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        let new_time = Utc::now();
        metadata.set_accessed_at(new_time);

        assert_ne!(metadata.get_accessed_at(), &original_atime);
        assert_eq!(metadata.get_accessed_at(), &new_time);
    }

    #[test]
    fn test_unix_ids() {
        let metadata = Metadata::new(ModeType::File);

        // Test default values match current user
        assert_eq!(metadata.get_uid(), get_current_uid());
        assert_eq!(metadata.get_gid(), get_current_gid());

        // Test setting custom values
        let mut metadata = Metadata::with_root_ownership(ModeType::File);
        assert_eq!(metadata.get_uid(), 0);
        assert_eq!(metadata.get_gid(), 0);

        metadata.set_uid(1000);
        metadata.set_gid(1000);

        assert_eq!(metadata.get_uid(), 1000);
        assert_eq!(metadata.get_gid(), 1000);

        // Test updating values
        metadata.set_uid(2000);
        metadata.set_gid(2000);

        assert_eq!(metadata.get_uid(), 2000);
        assert_eq!(metadata.get_gid(), 2000);
    }

    #[test]
    fn test_metadata_clone() {
        let mut original = Metadata::with_root_ownership(ModeType::File);
        original.set_uid(1000);
        original.set_gid(1000);
        original.set_size(100);

        let cloned = original.clone();

        assert_eq!(cloned.get_uid(), original.get_uid());
        assert_eq!(cloned.get_gid(), original.get_gid());
        assert_eq!(cloned.get_size(), original.get_size());
        assert_eq!(cloned.get_accessed_at(), original.get_accessed_at());
    }
}
