use zeroize::Zeroize;

/// Heap buffer that cryptographically wipes its contents on drop.
/// mlock is attempted best-effort on supported platforms; failure is non-fatal.
pub struct SecureBuffer {
    data: Vec<u8>,
}

impl SecureBuffer {
    pub fn new(data: Vec<u8>) -> Self {
        let buf = Self { data };
        buf.try_mlock();
        buf
    }

    pub fn from_slice(slice: &[u8]) -> Self {
        Self::new(slice.to_vec())
    }

    pub fn zeroed(len: usize) -> Self {
        Self::new(vec![0u8; len])
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[inline]
    fn try_mlock(&self) {
        if self.data.is_empty() {
            return;
        }
        #[cfg(unix)]
        unsafe {
            // Failure is silently ignored — best-effort security enhancement
            libc::mlock(
                self.data.as_ptr() as *const libc::c_void,
                self.data.len(),
            );
        }
        // Windows: VirtualLock requires SE_LOCK_MEMORY_PRIVILEGE; skip silently
    }
}

impl Drop for SecureBuffer {
    fn drop(&mut self) {
        #[cfg(unix)]
        if !self.data.is_empty() {
            unsafe {
                libc::munlock(
                    self.data.as_ptr() as *const libc::c_void,
                    self.data.len(),
                );
            }
        }
        self.data.zeroize();
    }
}

impl Clone for SecureBuffer {
    fn clone(&self) -> Self {
        Self::new(self.data.clone())
    }
}

// Safety: SecureBuffer wraps Vec<u8> which is Send + Sync
unsafe impl Send for SecureBuffer {}
unsafe impl Sync for SecureBuffer {}
