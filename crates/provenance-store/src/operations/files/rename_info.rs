//! The `FILE_RENAME_INFO` buffer of a Windows rename by handle.
use std::marker::PhantomData;
use std::mem::{align_of, offset_of, size_of};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, BorrowedHandle};
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem::{FILE_RENAME_INFO, FILE_RENAME_INFO_0};

// The buffer is a vector of `usize`, so its alignment is that of `usize`.
const _: () = assert!(align_of::<FILE_RENAME_INFO>() <= align_of::<usize>());

/// A `FILE_RENAME_INFO` with a new name, the flags of the rename, and the
/// directory that a relative name is in. The buffer lives no longer than
/// the handle of that directory.
pub(crate) struct RenameInfo<'root> {
    storage: Vec<usize>,
    size: u32,
    root: PhantomData<BorrowedHandle<'root>>,
}

impl<'root> RenameInfo<'root> {
    /// The structure for a rename to `name` with `flags`. With no `root`,
    /// `name` is a full path. The flags go in the field `Flags` of
    /// `FileRenameInfoEx`; for `FileRenameInformation`, the flag 1 is
    /// `ReplaceIfExists`.
    pub(crate) fn new(
        name: &std::ffi::OsStr,
        flags: u32,
        root: Option<BorrowedHandle<'root>>,
    ) -> std::io::Result<Self> {
        let name: Vec<u16> = name.encode_wide().collect();
        let name_bytes = name
            .len()
            .checked_mul(size_of::<u16>())
            .ok_or(std::io::ErrorKind::InvalidInput)?;
        let buffer_size = offset_of!(FILE_RENAME_INFO, FileName)
            .checked_add(name_bytes)
            .and_then(|size| size.checked_add(size_of::<u16>()))
            .ok_or(std::io::ErrorKind::InvalidInput)?;
        let name_length = u32::try_from(name_bytes).map_err(|_| std::io::ErrorKind::InvalidInput)?;
        let size = u32::try_from(buffer_size).map_err(|_| std::io::ErrorKind::InvalidInput)?;
        let mut storage = vec![0_usize; buffer_size.div_ceil(size_of::<usize>())];
        let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        let root_directory = root.map_or(0, |handle| handle.as_raw_handle() as HANDLE);
        // SAFETY: Size: `storage` has at least `buffer_size` bytes, the offset
        // of `FileName`, the name, and one `u16` more. Alignment: `storage`
        // has the alignment of `usize`, and the assert above this type shows
        // that it is enough for `FILE_RENAME_INFO`. Terminator: `vec!` fills
        // `storage` with zeros, and the copy writes only `name.len()` units
        // from the offset of `FileName`, so the last `u16` stays zero.
        // Handle: `root` borrows the handle for `'root`, and `Self` keeps
        // that borrow, so the handle stays open while the buffer exists.
        unsafe {
            info.write(FILE_RENAME_INFO {
                Anonymous: FILE_RENAME_INFO_0 { Flags: flags },
                RootDirectory: root_directory,
                FileNameLength: name_length,
                FileName: [0],
            });
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
                name.len(),
            );
        }
        Ok(Self {
            storage,
            size,
            root: PhantomData,
        })
    }

    /// The address of the structure, for the call that does the rename.
    pub(crate) fn as_ptr(&self) -> *const std::ffi::c_void {
        self.storage.as_ptr().cast()
    }

    /// The length of the buffer in bytes, for the call that does the rename.
    pub(crate) const fn size(&self) -> u32 {
        self.size
    }
}
