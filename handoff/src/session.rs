use crate::envelope::Envelope;
use ring::aead::Aad;
use ring::aead::LessSafeKey;
use ring::aead::Nonce;
use ring::aead::UnboundKey;
use ring::aead::{self};
use zeroize::Zeroizing;
/// Ephemeral shared authorization; Tailscale reachability alone never authorizes a request.
pub struct Session {
    key: Zeroizing<[u8; 32]>,
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Session([redacted])")
    }
}
impl Session {
    pub fn from_bootstrap(text: &str) -> eyre::Result<Self> {
        let bytes = crate::wire::unhex(text)?;
        let key = bytes
            .as_slice()
            .try_into()
            .map_err(|_redacted_error| eyre::eyre!("invalid session"))?;
        Ok(Self {
            key: Zeroizing::new(key),
        })
    }
    pub fn random() -> eyre::Result<Self> {
        Ok(Self {
            key: Zeroizing::new(crate::wire::random()?),
        })
    }
    /// Human bootstrap is displayed in a native desktop dialog, never logs or files.
    pub fn bootstrap(&self) -> Zeroizing<String> {
        Zeroizing::new(crate::wire::hex(self.key.as_ref()))
    }
    /// Temporary raw key copy for delivery only inside an already mutually confirmed channel.
    /// No immutable string copy is created; returned bytes clear on drop best effort.
    pub fn transport_key(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(*self.key)
    }
    fn aead(&self) -> eyre::Result<LessSafeKey> {
        Ok(LessSafeKey::new(
            UnboundKey::new(&aead::AES_256_GCM, self.key.as_ref())
                .map_err(|_redacted_error| eyre::eyre!("invalid session"))?,
        ))
    }
    pub fn seal(&self, aad: &[u8], plaintext: &[u8]) -> eyre::Result<Envelope> {
        eyre::ensure!(plaintext.len() <= 8192, "oversized message");
        let nonce = crate::wire::random::<12>()?;
        let mut bytes = Zeroizing::new(plaintext.to_vec());
        self.aead()?
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad),
                &mut *bytes,
            )
            .map_err(|_redacted_error| eyre::eyre!("authentication failed"))?;
        Ok(Envelope {
            nonce: crate::wire::hex(&nonce),
            ciphertext: crate::wire::hex(&bytes),
        })
    }
    pub fn open(&self, aad: &[u8], envelope: &Envelope) -> eyre::Result<Zeroizing<Vec<u8>>> {
        let nonce = crate::wire::unhex(&envelope.nonce)?;
        let nonce: [u8; 12] = nonce
            .as_slice()
            .try_into()
            .map_err(|_redacted_error| eyre::eyre!("authentication failed"))?;
        let mut bytes = crate::wire::unhex(&envelope.ciphertext)?;
        eyre::ensure!(bytes.len() <= 8208, "oversized message");
        let len = self
            .aead()?
            .open_in_place(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad),
                &mut bytes,
            )
            .map_err(|_redacted_error| eyre::eyre!("authentication failed"))?
            .len();
        bytes.truncate(len);
        Ok(bytes)
    }
}
