use super::RepositoryFileRefusal as Refusal;
use std::fs::{File, Permissions};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct FileIdentity {
    pub(super) volume: u64,
    pub(super) file: u64,
}

pub(super) struct FileMetadata {
    pub permissions: Permissions,
    #[cfg(unix)]
    owner_uid: u32,
    #[cfg(windows)]
    dacl: WindowsDacl,
}

impl FileMetadata {
    pub fn read(file: &File) -> Result<Self, Refusal> {
        let metadata = file.metadata().map_err(Refusal::Read)?;
        Ok(Self {
            permissions: metadata.permissions(),
            #[cfg(unix)]
            owner_uid: std::os::unix::fs::MetadataExt::uid(&metadata),
            #[cfg(windows)]
            dacl: WindowsDacl::read(file)?,
        })
    }

    pub fn apply(&self, file: &File) -> Result<(), Refusal> {
        #[cfg(unix)]
        {
            let effective = effective_uid();
            if self.owner_uid != effective {
                return Err(Refusal::OwnerMismatch {
                    owner: self.owner_uid,
                    effective,
                });
            }
        }
        file.set_permissions(self.permissions.clone())
            .map_err(Refusal::Write)?;
        #[cfg(windows)]
        self.dacl.apply(file)?;
        Ok(())
    }
}

pub(super) fn identity(file: &File) -> Result<FileIdentity, Refusal> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let metadata = file.metadata().map_err(Refusal::Read)?;
        Ok(FileIdentity {
            volume: metadata.dev(),
            file: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        use std::mem::MaybeUninit;
        use std::os::windows::io::AsRawHandle as _;
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };
        let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        if unsafe {
            GetFileInformationByHandle(file.as_raw_handle() as HANDLE, information.as_mut_ptr())
        } == 0
        {
            return Err(Refusal::Read(std::io::Error::last_os_error()));
        }
        let information = unsafe { information.assume_init() };
        Ok(FileIdentity {
            volume: u64::from(information.dwVolumeSerialNumber),
            file: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        })
    }
}

#[cfg(unix)]
fn effective_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

#[cfg(windows)]
struct WindowsDacl(Option<Vec<u8>>);

#[cfg(windows)]
impl WindowsDacl {
    #[cfg(windows)]
    fn read(file: &File) -> Result<Self, Refusal> {
        use std::mem::MaybeUninit;
        use std::os::windows::io::AsRawHandle as _;
        use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS, HANDLE, HLOCAL};
        use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
        use windows_sys::Win32::Security::{
            AclSizeInformation, GetAclInformation, ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION,
            PSECURITY_DESCRIPTOR,
        };

        let mut dacl = std::ptr::null_mut();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle() as HANDLE,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(Refusal::Write(std::io::Error::from_raw_os_error(
                status as i32,
            )));
        }
        let result = if dacl.is_null() {
            Ok(Self(None))
        } else {
            let mut info = MaybeUninit::<ACL_SIZE_INFORMATION>::uninit();
            if unsafe {
                GetAclInformation(
                    dacl,
                    info.as_mut_ptr().cast(),
                    u32::try_from(std::mem::size_of::<ACL_SIZE_INFORMATION>()).unwrap(),
                    AclSizeInformation,
                )
            } == 0
            {
                Err(Refusal::Write(std::io::Error::last_os_error()))
            } else {
                let length = unsafe { info.assume_init() }.AclBytesInUse as usize;
                let bytes = unsafe { std::slice::from_raw_parts(dacl.cast::<u8>(), length) };
                Ok(Self(Some(bytes.to_vec())))
            }
        };
        unsafe { LocalFree(descriptor as HLOCAL) };
        result
    }

    #[cfg(windows)]
    fn apply(&self, file: &File) -> Result<(), Refusal> {
        use std::os::windows::io::AsRawHandle as _;
        use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
        use windows_sys::Win32::Security::Authorization::{SetSecurityInfo, SE_FILE_OBJECT};
        use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

        let dacl = self
            .0
            .as_ref()
            .map_or(std::ptr::null(), |bytes| bytes.as_ptr().cast());
        let status = unsafe {
            SetSecurityInfo(
                file.as_raw_handle() as HANDLE,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                dacl,
                std::ptr::null(),
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(Refusal::Write(std::io::Error::from_raw_os_error(
                status as i32,
            )))
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    #[test]
    fn metadata_refuses_an_owner_other_than_the_effective_user() {
        let file = tempfile::tempfile().unwrap();
        let metadata = FileMetadata {
            permissions: Permissions::from_mode(0o600),
            owner_uid: effective_uid().wrapping_add(1),
        };

        assert!(matches!(
            metadata.apply(&file),
            Err(Refusal::OwnerMismatch { .. })
        ));
    }
}
