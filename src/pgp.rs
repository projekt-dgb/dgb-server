use std::io::{self, Write};

use sequoia_openpgp::cert::prelude::*;
use sequoia_openpgp::parse::PacketParser;
use sequoia_openpgp::parse::{stream::*, Parse};
use sequoia_openpgp::policy::Policy;

pub fn parse_cert(cert: &[u8]) -> Result<sequoia_openpgp::Cert, String> {
    let ppr = PacketParser::from_bytes(cert).map_err(|e| format!("{e}"))?;

    sequoia_openpgp::Cert::try_from(ppr).map_err(|e| format!("{e}"))
}

pub fn generate(user_id: &str) -> sequoia_openpgp::Result<sequoia_openpgp::Cert> {
    let (cert, _revocation) = CertBuilder::new()
        .add_userid(user_id)
        .add_signing_subkey()
        .generate()?;

    Ok(cert)
}

/// Verifies the given message.
pub fn verify(
    p: &dyn Policy,
    sink: &mut dyn Write,
    signed_message: &[u8],
    sender: &sequoia_openpgp::Cert,
) -> sequoia_openpgp::Result<()> {
    // Make a helper that that feeds the sender's public key to the
    // verifier.
    let helper = Helper { cert: sender };

    // Now, create a verifier with a helper using the given Certs.
    let mut verifier = VerifierBuilder::from_bytes(signed_message)?.with_policy(p, None, helper)?;

    // Verify the data.
    io::copy(&mut verifier, sink)?;

    Ok(())
}

struct Helper<'a> {
    cert: &'a sequoia_openpgp::Cert,
}

impl<'a> VerificationHelper for Helper<'a> {
    fn get_certs(
        &mut self,
        _ids: &[sequoia_openpgp::KeyHandle],
    ) -> sequoia_openpgp::Result<Vec<sequoia_openpgp::Cert>> {
        // Return public keys for signature verification here.
        Ok(vec![self.cert.clone()])
    }

    fn check(&mut self, structure: MessageStructure) -> sequoia_openpgp::Result<()> {
        // In this function, we implement our signature verification
        // policy.

        let mut good = false;
        for (i, layer) in structure.into_iter().enumerate() {
            match (i, layer) {
                // First, we are interested in signatures over the
                // data, i.e. level 0 signatures.
                (0, MessageLayer::SignatureGroup { results }) => {
                    // Finally, given a VerificationResult, which only says
                    // whether the signature checks out mathematically, we apply
                    // our policy.
                    match results.into_iter().next() {
                        Some(Ok(_)) => good = true,
                        Some(Err(e)) => return Err(sequoia_openpgp::Error::from(e).into()),
                        None => return Err(anyhow::anyhow!("No signature")),
                    }
                }
                _ => return Err(anyhow::anyhow!("Unexpected message structure")),
            }
        }

        if good {
            Ok(()) // Good signature.
        } else {
            Err(anyhow::anyhow!("Signature verification failed"))
        }
    }
}
