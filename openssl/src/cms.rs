//! SMIME implementation using CMS
//!
//! CMS (PKCS#7) is an encyption standard.  It allows signing and ecrypting data using
//! X.509 certificates.  The OpenSSL implementation of CMS is used in email encryption
//! generated from a `Vec` of bytes.  This `Vec` follows the smime protocol standards.
//! Data accepted by this module will be smime type `enveloped-data`.

use ffi;
use foreign_types::{ForeignType, ForeignTypeRef};
use std::ptr;

use bio::{MemBio, MemBioSlice};
use error::ErrorStack;
use libc::c_uint;
use pkey::{HasPrivate, PKeyRef};
use stack::StackRef;
use x509::store::{ X509StoreRef};
use x509::{X509Ref, X509};
use symm::Cipher;
use {cvt, cvt_p};

bitflags! {
    pub struct CMSOptions : c_uint {
        const TEXT = ffi::CMS_TEXT;
        const CMS_NOCERTS = ffi::CMS_NOCERTS;
        const NO_CONTENT_VERIFY = ffi::CMS_NO_CONTENT_VERIFY;
        const NO_ATTR_VERIFY = ffi::CMS_NO_ATTR_VERIFY;
        const NOSIGS = ffi::CMS_NOSIGS;
        const NOINTERN = ffi::CMS_NOINTERN;
        const NO_SIGNER_CERT_VERIFY = ffi::CMS_NO_SIGNER_CERT_VERIFY;
        const NOVERIFY = ffi::CMS_NOVERIFY;
        const DETACHED = ffi::CMS_DETACHED;
        const BINARY = ffi::CMS_BINARY;
        const NOATTR = ffi::CMS_NOATTR;
        const NOSMIMECAP = ffi::CMS_NOSMIMECAP;
        const NOOLDMIMETYPE = ffi::CMS_NOOLDMIMETYPE;
        const CRLFEOL = ffi::CMS_CRLFEOL;
        const STREAM = ffi::CMS_STREAM;
        const NOCRL = ffi::CMS_NOCRL;
        const PARTIAL = ffi::CMS_PARTIAL;
        const REUSE_DIGEST = ffi::CMS_REUSE_DIGEST;
        const USE_KEYID = ffi::CMS_USE_KEYID;
        const DEBUG_DECRYPT = ffi::CMS_DEBUG_DECRYPT;
        #[cfg(all(not(libressl), not(ossl101)))]
        const KEY_PARAM = ffi::CMS_KEY_PARAM;
        #[cfg(all(not(libressl), not(ossl101), not(ossl102)))]
        const ASCIICRLF = ffi::CMS_ASCIICRLF;
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::CMS_ContentInfo;
    fn drop = ffi::CMS_ContentInfo_free;

    /// High level CMS wrapper
    ///
    /// CMS supports nesting various types of data, including signatures, certificates,
    /// encrypted data, smime messages (encrypted email), and data digest.  The ContentInfo
    /// content type is the encapsulation of all those content types.  [`RFC 5652`] describes
    /// CMS and OpenSSL follows this RFC's implmentation.
    ///
    /// [`RFC 5652`]: https://tools.ietf.org/html/rfc5652#page-6
    pub struct CmsContentInfo;
    /// Reference to [`CMSContentInfo`]
    ///
    /// [`CMSContentInfo`]:struct.CmsContentInfo.html
    pub struct CmsContentInfoRef;
}

impl CmsContentInfoRef {
    /// Given the sender's private key, `pkey` and the recipient's certificiate, `cert`,
    /// decrypt the data in `self`.
    ///
    /// OpenSSL documentation at [`CMS_decrypt`]
    ///
    /// [`CMS_decrypt`]: https://www.openssl.org/docs/man1.1.0/crypto/CMS_decrypt.html
    pub fn decrypt<T>(&self, pkey: &PKeyRef<T>, cert: &X509) -> Result<Vec<u8>, ErrorStack>
    where
        T: HasPrivate,
    {
        unsafe {
            let pkey = pkey.as_ptr();
            let cert = cert.as_ptr();
            let out = MemBio::new()?;
            let flags: u32 = 0;

            cvt(ffi::CMS_decrypt(
                self.as_ptr(),
                pkey,
                cert,
                ptr::null_mut(),
                out.as_ptr(),
                flags.into(),
            ))?;

            Ok(out.get_buf().to_owned())
        }
    }

    to_der! {
        /// Serializes this CmsContentInfo using DER.
        ///
        /// OpenSSL documentation at [`i2d_CMS_ContentInfo`]
        ///
        /// [`i2d_CMS_ContentInfo`]: https://www.openssl.org/docs/man1.0.2/crypto/i2d_CMS_ContentInfo.html
        to_der,
        ffi::i2d_CMS_ContentInfo
    }

    to_pem! {
        /// Serializes this CmsContentInfo using DER.
        ///
        /// OpenSSL documentation at [`PEM_write_bio_CMS`]
        ///
        /// [`PEM_write_bio_CMS`]: https://www.openssl.org/docs/man1.1.0/man3/PEM_write_bio_CMS.html
        to_pem,
        ffi::PEM_write_bio_CMS
    }
}

impl CmsContentInfo {
    /// Parses a smime formatted `vec` of bytes into a `CmsContentInfo`.
    ///
    /// OpenSSL documentation at [`SMIME_read_CMS`]
    ///
    /// [`SMIME_read_CMS`]: https://www.openssl.org/docs/man1.0.2/crypto/SMIME_read_CMS.html
    pub fn smime_read_cms(smime: &[u8]) -> Result<CmsContentInfo, ErrorStack> {
        unsafe {
            let bio = MemBioSlice::new(smime)?;

            let cms = cvt_p(ffi::SMIME_read_CMS(bio.as_ptr(), ptr::null_mut()))?;

            Ok(CmsContentInfo::from_ptr(cms))
        }
    }

    from_der! {
        /// Deserializes a DER-encoded ContentInfo structure.
        ///
        /// This corresponds to [`d2i_CMS_ContentInfo`].
        ///
        /// [`d2i_CMS_ContentInfo`]: https://www.openssl.org/docs/manmaster/man3/d2i_X509.html
        from_der,
        CmsContentInfo,
        ffi::d2i_CMS_ContentInfo
    }

    from_pem! {
        /// Deserializes a PEM-encoded ContentInfo structure.
        ///
        /// This corresponds to [`PEM_read_bio_CMS`].
        ///
        /// [`PEM_read_bio_CMS`]: https://www.openssl.org/docs/man1.1.0/man3/PEM_read_bio_CMS.html
        from_pem,
        CmsContentInfo,
        ffi::PEM_read_bio_CMS
    }

    /// Given a signing cert `signcert`, private key `pkey`, a certificate stack `certs`,
    /// data `data` and flags `flags`, create a CmsContentInfo struct.
    ///
    /// All arguments are optional.
    ///
    /// OpenSSL documentation at [`CMS_sign`]
    ///
    /// [`CMS_sign`]: https://www.openssl.org/docs/manmaster/man3/CMS_sign.html
    pub fn sign<T>(
        signcert: Option<&X509Ref>,
        pkey: Option<&PKeyRef<T>>,
        certs: Option<&StackRef<X509>>,
        data: Option<&[u8]>,
        flags: CMSOptions,
    ) -> Result<CmsContentInfo, ErrorStack>
    where
        T: HasPrivate,
    {
        unsafe {
            let signcert = signcert.map_or(ptr::null_mut(), |p| p.as_ptr());
            let pkey = pkey.map_or(ptr::null_mut(), |p| p.as_ptr());
            let data_bio = match data {
                Some(data) => Some(MemBioSlice::new(data)?),
                None => None,
            };
            let data_bio_ptr = data_bio.as_ref().map_or(ptr::null_mut(), |p| p.as_ptr());
            let certs = certs.map_or(ptr::null_mut(), |p| p.as_ptr());

            let cms = cvt_p(ffi::CMS_sign(
                signcert,
                pkey,
                certs,
                data_bio_ptr,
                flags.bits(),
            ))?;

            Ok(CmsContentInfo::from_ptr(cms))
        }
    }

    /// Given a certificate stack `certs`, data `data`, cipher `cipher` and flags `flags`,
    /// create a CmsContentInfo struct.
    ///
    /// OpenSSL documentation at [`CMS_encrypt`]
    ///
    /// [`CMS_encrypt`]: https://www.openssl.org/docs/manmaster/man3/CMS_encrypt.html
    pub fn encrypt(
        certs: &StackRef<X509>,
        data: &[u8],
        cipher: Cipher,
        flags: CMSOptions,
    ) -> Result<CmsContentInfo, ErrorStack>
    {
        unsafe {
            let data_bio = MemBioSlice::new(data)?;

            let cms = cvt_p(ffi::CMS_encrypt(
                certs.as_ptr(),
                data_bio.as_ptr(),
                cipher.as_ptr(),
                flags.bits(),
            ))?;

            Ok(CmsContentInfo::from_ptr(cms))
        }
    }


    pub fn verify(
        &self,
        certs: &StackRef<X509>,
        store: &X509StoreRef,
        data: Option<&[u8]>,
        out: Option<&mut Vec<u8>>,
        flags: CMSOptions,
    ) -> Result<(), ErrorStack>
    {
        let out_bio = MemBio::new()?;

        let indata_bio = match data {
            Some(data) => Some(MemBioSlice::new(data)?),
            None => None,
        };
        let indata_bio_ptr = indata_bio.as_ref().map_or(ptr::null_mut(), |p| p.as_ptr());

        unsafe {
            cvt(ffi::CMS_verify(
                self.as_ptr(),
                certs.as_ptr(),
                store.as_ptr(),
                indata_bio_ptr,
                out_bio.as_ptr(),
                flags.bits,
            ))
            .map(|_| ())?
        }

        if let Some(data) = out {
            data.clear();
            data.extend_from_slice(out_bio.get_buf());
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::ffi::{CString};
    use stack::Stack;
    use x509::X509;
    use x509::store::{X509Store, X509StoreBuilder};
    use pkcs12::Pkcs12;
    use std::io::{self, Write};

    #[allow(dead_code)]	
    unsafe fn err_print_errors() -> Result<(), io::Error> {
        let out_bio = MemBio::new()?;
        ffi::ERR_print_errors( out_bio.as_ptr() );
        io::stdout().write_all( out_bio.get_buf() )?;
        Ok(())
    }

    #[test]
    fn cms_verify() {
        let mut outdata = Vec::new();

        let ca_cert_bytes = include_bytes!("../test/cms_verify_ca.pem");
        let p7_cert_bytes = include_bytes!("../test/cms_verify_p7.pem");

        let ca_cert = X509::from_pem(ca_cert_bytes).expect("failed to load ca cert");
        let cert_stack = Stack::new().expect("failed to create stack");
        
        let mut builder = X509StoreBuilder::new().unwrap();
        let _ = builder.add_cert(ca_cert);

        let store: X509Store = builder.build();

        let cms = CmsContentInfo::from_pem(p7_cert_bytes).expect("failed create CMS from pem");
        cms.verify(
            &cert_stack, 
            &store, 
            None, 
            Some(&mut outdata), 
            CMSOptions::empty() 
        ).expect("cms verification faild");

    }

    #[test]
    fn cms_verify_gost() {
        let mut outdata = Vec::new();
        unsafe{

            ffi::ENGINE_load_builtin_engines();
            ffi::OPENSSL_load_builtin_modules();

            /*let mut app = CString::new("app").expect("failed");
            
            let conf = CString::new("../test/openssl.cfg").expect("failed");
            
            let res = ffi::CONF_modules_load_file(conf.as_ptr() , app.as_ptr(), 0x0 as  c_uint);
            if(res<=0){
                ERR_print_errors();
                println!("res {}", res);
            }
            assert!(res==0x01); 
            */

            let app  = CString::new("gost").expect("unexpected failed");
            let e=ffi::ENGINE_by_id(app.as_ptr());

            assert!(!e.is_null());
        }
    
        let  ca_cert_bytes = include_bytes!("../test/cms_verify_ca_gost.pem");
        
        let mut ca_cert = X509::from_pem(ca_cert_bytes).expect("failed to load ca cert");
        let cert_stack = Stack::new().expect("failed to create stack");
        
        let mut builder = X509StoreBuilder::new().unwrap();
        builder.add_cert(ca_cert).expect("unexpected");
        //second ca
        let ca_cert_bytes2 = include_bytes!("../test/cms_verify_ca_gost2.pem");
        ca_cert = X509::from_pem(ca_cert_bytes2).expect("failed to load ca cert");
        builder.add_cert(ca_cert).expect("unexpected");

        let store: X509Store = builder.build();
        let p7_cert_bytes = include_bytes!("../test/cms_verify_p7_gost.pem");
        let cms = CmsContentInfo::from_pem(p7_cert_bytes).expect("failed create CMS from pem");
        
        cms.verify(
            &cert_stack, 
            &store, 
            None, 
            Some(&mut outdata), 
            CMSOptions::empty() 
        ).expect("cms verification faild");
    }

    #[test]
    fn cms_encrypt_decrypt() {
        // load cert with public key only
        let pub_cert_bytes = include_bytes!("../test/cms_pubkey.der");
        let pub_cert = X509::from_der(pub_cert_bytes).expect("failed to load pub cert");

        // load cert with private key
        let priv_cert_bytes = include_bytes!("../test/cms.p12");
        let priv_cert = Pkcs12::from_der(priv_cert_bytes).expect("failed to load priv cert");
        let priv_cert = priv_cert.parse("mypass").expect("failed to parse priv cert");

        // encrypt cms message using public key cert
        let input = String::from("My Message");
        let mut cert_stack = Stack::new().expect("failed to create stack");
        cert_stack.push(pub_cert).expect("failed to add pub cert to stack");

        let encrypt = CmsContentInfo::encrypt(&cert_stack, &input.as_bytes(), Cipher::des_ede3_cbc(), CMSOptions::empty())
            .expect("failed create encrypted cms");

        // decrypt cms message using private key cert (DER)
        {
            let encrypted_der = encrypt.to_der().expect("failed to create der from cms");
            let decrypt = CmsContentInfo::from_der(&encrypted_der).expect("failed read cms from der");
            let decrypt = decrypt.decrypt(&priv_cert.pkey, &priv_cert.cert).expect("failed to decrypt cms");
            let decrypt = String::from_utf8(decrypt).expect("failed to create string from cms content");
            assert_eq!(input, decrypt);
        }

        // decrypt cms message using private key cert (PEM)
        {
            let encrypted_pem = encrypt.to_pem().expect("failed to create pem from cms");
            let decrypt = CmsContentInfo::from_pem(&encrypted_pem).expect("failed read cms from pem");
            let decrypt = decrypt.decrypt(&priv_cert.pkey, &priv_cert.cert).expect("failed to decrypt cms");
            let decrypt = String::from_utf8(decrypt).expect("failed to create string from cms content");
            assert_eq!(input, decrypt);
        }
    }
}
