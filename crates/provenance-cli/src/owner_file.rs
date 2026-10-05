//! Windows files whose protected access list grants rights only to the current user.

use std::{
    ffi::c_void,
    fs::File,
    io,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt as _,
        io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    },
    path::Path,
    ptr::{addr_of_mut, null_mut},
};
use windows_sys::Win32::{
    Foundation::{LocalFree, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT,
        },
        EqualSid, GetAce, GetSecurityDescriptorControl, GetSecurityDescriptorOwner,
        GetTokenInformation, TokenUser, ACCESS_ALLOWED_ACE, DACL_SECURITY_INFORMATION,
        OWNER_SECURITY_INFORMATION, SECURITY_ATTRIBUTES, SE_DACL_PROTECTED, TOKEN_QUERY,
        TOKEN_USER,
    },
    Storage::FileSystem::{
        CreateFileW, CREATE_NEW, FILE_ALL_ACCESS, FILE_ATTRIBUTE_NORMAL,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    },
    System::{
        SystemServices::ACCESS_ALLOWED_ACE_TYPE,
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

struct LocalMemory(*mut c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // The Windows security functions allocate these buffers with LocalAlloc.
        unsafe { LocalFree(self.0) };
    }
}

fn checked(result: i32) -> io::Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn current_user_sid() -> io::Result<String> {
    // The token and aligned buffer remain live until the SID has been copied.
    unsafe {
        let mut token = 0;
        checked(OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY,
            &mut token,
        ))?;
        let token = OwnedHandle::from_raw_handle(token as *mut c_void);
        let mut length = 0;
        GetTokenInformation(
            token.as_raw_handle() as isize,
            TokenUser,
            null_mut(),
            0,
            &mut length,
        );
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
        checked(GetTokenInformation(
            token.as_raw_handle() as isize,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        ))?;
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid = null_mut();
        checked(ConvertSidToStringSidW(user.User.Sid, &mut sid))?;
        let _allocation = LocalMemory(sid.cast());
        let mut length = 0;
        while *sid.add(length) != 0 {
            length += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(sid, length))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

fn owner_descriptor() -> io::Result<LocalMemory> {
    let sid = current_user_sid()?;
    let descriptor: Vec<u16> = format!("O:{sid}D:P(A;;FA;;;{sid})\0")
        .encode_utf16()
        .collect();
    let mut security = null_mut();
    // The descriptor is terminated, and Windows returns an owned allocation.
    unsafe {
        checked(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            SDDL_REVISION_1,
            &mut security,
            null_mut(),
        ))?;
    }
    Ok(LocalMemory(security))
}

/// Creates and checks an owner-only file before the caller can write a secret.
pub fn create(path: &Path) -> io::Result<File> {
    let security = owner_descriptor()?;
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the file path contains a null",
        ));
    }
    name.push(0);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    // CREATE_NEW refuses existing paths; all input buffers remain live for the call.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // A successful CreateFileW call transfers this handle to File.
    let file = unsafe { File::from_raw_handle(handle as *mut c_void) };
    verify_with(&file, &security)?;
    Ok(file)
}

/// Checks an open file before the caller reads a saved secret.
pub fn verify(file: &File) -> io::Result<()> {
    verify_with(file, &owner_descriptor()?)
}

fn verify_with(file: &File, expected: &LocalMemory) -> io::Result<()> {
    // All SID and ACL pointers refer to the two live security descriptors.
    unsafe {
        let mut owner = null_mut();
        let mut acl = null_mut();
        let mut descriptor = null_mut();
        let status = GetSecurityInfo(
            file.as_raw_handle() as isize,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut descriptor,
        );
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let descriptor = LocalMemory(descriptor);
        let mut control = 0;
        let mut revision = 0;
        checked(GetSecurityDescriptorControl(
            descriptor.0,
            &mut control,
            &mut revision,
        ))?;
        let mut expected_owner = null_mut();
        let mut defaulted = 0;
        checked(GetSecurityDescriptorOwner(
            expected.0,
            &mut expected_owner,
            &mut defaulted,
        ))?;
        if owner.is_null()
            || control & SE_DACL_PROTECTED == 0
            || acl.is_null()
            || (*acl).AceCount != 1
            || EqualSid(owner, expected_owner) == 0
        {
            return Err(access_error());
        }
        let mut ace = null_mut();
        checked(GetAce(acl, 0, &mut ace))?;
        let ace = ace.cast::<ACCESS_ALLOWED_ACE>();
        if (*ace).Header.AceType as u32 != ACCESS_ALLOWED_ACE_TYPE
            || (*ace).Header.AceFlags != 0
            || ((*ace).Header.AceSize as usize) < size_of::<ACCESS_ALLOWED_ACE>()
            || (*ace).Mask != FILE_ALL_ACCESS
            || EqualSid(addr_of_mut!((*ace).SidStart).cast(), expected_owner) == 0
        {
            return Err(access_error());
        }
        Ok(())
    }
}

fn access_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "the review file does not have a protected owner-only ACL",
    )
}
