\newpage
\twocolumngrid
# Appendix B: Additional Specifications

## Secure Private Key Management (lair-keystore)

Holochain implementations MUST provide a secure way to create, manage and use public/private key pairs, as well as store them encrypted at rest. Implementations MAY vary how the Conductor connects to the keystore (e.g., including in the same process or communicating over a secure channel). The full specification of key the keystore API is beyond the scope of this document; see the [Lair repository on GitHub](https://github.com/holochain/lair) for our full implementation. However, we note that the API MUST be sufficient to service the following calls via the HDK and Conductor API:

* `CreateKeyPair() -> PubKey`
* `RegisterKeyPair(PrivKey, PubKey)`
* `Sign(Vec<u8>, PubKey) -> Vec<u8>`
* `VerifySignature(Vec<u8>, PubKey) -> bool`
* `Encrypt(Vec<u8>, PubKey) -> Vec<u8>`
* `Decrypt(Vec<u8>, PubKey) -> Vec<u8>`

## External References and Holochain Resource Locators

As development frameworks or protocols and their ecosystems mature, there arise demands for common conventions for data interoperation. In the Holochain ecosystem, a desire for a common convention for referring to data in other DHTs has emerged. This has led to the development of the Holochain Resource Locator (HRL). This exists as a data pattern which applications can implement and adhere to. Refer to the [HRL Design Document](https://hackmd.io/@hololtd/HyWnqhTnY?type=view) for details.