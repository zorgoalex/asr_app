use anyhow::{anyhow, Result};
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(
            &mut in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| anyhow!("CryptProtectData failed: {e}"))?;
        let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        LocalFree(HLOCAL(out_blob.pbData as *mut std::ffi::c_void));
        Ok(out)
    }
}

pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let mut in_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(
            &mut in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| anyhow!("CryptUnprotectData failed: {e}"))?;
        let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        LocalFree(HLOCAL(out_blob.pbData as *mut std::ffi::c_void));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_roundtrip() {
        let input = b"hello-world";
        let enc = protect(input).unwrap();
        let dec = unprotect(&enc).unwrap();
        assert_eq!(dec, input);
    }
}
