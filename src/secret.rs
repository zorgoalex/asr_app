use anyhow::{anyhow, Result};
use windows::Win32::Foundation::LocalFree;
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPTOAPI_BLOB,
};

pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let mut in_blob = CRYPTOAPI_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPTOAPI_BLOB::default();
        let ok = CryptProtectData(
            &mut in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        );
        if !ok.as_bool() {
            return Err(anyhow!("CryptProtectData failed"));
        }
        let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        LocalFree(out_blob.pbData as isize);
        Ok(out)
    }
}

pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let mut in_blob = CRYPTOAPI_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPTOAPI_BLOB::default();
        let ok = CryptUnprotectData(
            &mut in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        );
        if !ok.as_bool() {
            return Err(anyhow!("CryptUnprotectData failed"));
        }
        let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        LocalFree(out_blob.pbData as isize);
        Ok(out)
    }
}
