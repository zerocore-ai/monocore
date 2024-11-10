# did:wk Method Specification

> [!WARNING]
> This specification is still a work in progress and may be subject to change. Feedback and contributions are welcome.

### Authors

- [Stephen Akinyemi][steve-github]

### Version

`0.1.0`

## Table of Contents

1. [Introduction](#1-introduction)
2. [The `did:wk` Format](#2-the-didwk-format)

   1. [Method Name](#21-method-name)
   2. [Target System](#22-target-system)
   3. [Method Specific Identifier](#23-method-specific-identifier)

3. [Operations](#3-operations)

   1. [Create](#31-create)
      1. [Document Creation](#311-document-creation)
      2. [Signature Method Creation](#312-signature-method-creation)
      3. [Decode Public Key](#313-decode-public-key)
      4. [Encode Public Key](#314-encode-public-key)
   2. [Read](#32-read)
   3. [Update](#33-update)
   4. [Deactivate](#34-deactivate)

4. [Security & Privacy Considerations](#4-security--privacy-considerations)
   1. [Key Rotation](#41-key-rotation)
   2. [Key Revocation](#42-key-revocation)
   3. [Caveats](#43-caveats)

## 1. Introduction

Decentralized Identifiers (DIDs) provide a self-sovereign way for individuals and organizations to manage identity information in a secure and privacy-preserving manner. The `did:wk` method builds upon existing DID concepts and leverages the widely established infrastructure of the web to offer a decentralized solution that prioritizes user control and robust verification.

**Motivation**

While DID methods like `did:key` [^1] offer simplicity, they lack a robust mechanism to establish verifiable ownership of the underlying key pair. Conversely, `did:web` [^2] relies on domain names, but doesn't offer an intrinsic way to prove the connection between the DID document and the controller of the domain. The `did:wk` method aims to bridge this gap by:

- **Strong Ownership Proof:** Incorporating a cryptographic signature within the DID document to establish a verifiable link between the document and the corresponding public key.
- **Decentralized Foundation:** Utilizing user-owned web servers to promote self-sovereignty, giving users control over their DID documents.
- **Flexibility:** Optionally linking to a hosted DID document for richer metadata and features, while also functioning directly as a key-based DID when needed.

**Target Audience**

The `did:wk` method is designed for:

- **Individual Users** seeking enhanced control over their online identity.
- **Organizations** wanting a self-hosted, verifiable approach to digital identity.

## 2. The `did:wk` Format

### 2.1 Method Name

The method name for this specification is `wk`. The abbreviation 'wk' stands for "web key".

### 2.2 Target System

The `did:wk` method is designed to be used within decentralized systems and applications that can resolve URLs within the HTTPS scheme to locate DID documents. These systems may be built on top of distributed ledger technologies, peer-to-peer networks, or traditional web architectures.

### 2.3 Method Specific Identifier

The method-specific identifier of a `did:wk` DID is comprised of the following components:

- **Scheme:** `did:wk:`
- **Encoded Public Key:** A public key, encoded using:
  1. **Multicodec** to specify the key type and format.
  2. **Multibase** to represent the encoded key as a string.
- **Optional Locator Component:** Comprised of:
  - The `@` symbol as a separator.
  - The **host** portion of a domain name.
  - An optional **port** component preceded by a `:`.
  - An optional **path** component for more specific resource locations.

Here is the grammar for the `did:wk` format:

```abnf
did-wk             = "did:wk:" multibase-key [ "@" locator-component ]
locator-component  = <host> [ ":" <port> ] [ <path-abempty> ]
multibase-key      = <MULTIBASE(base-encoding-type, MULTICODEC(public-key-type, raw-public-key-bytes))>
```

`<host>`, `<port>` and `<path-abempty>` are defined as per the URI specification [^3].

`<MULTIBASE>` and `<MULTICODEC>` are defined in the Multiformats [^4] specification.

**Examples:**

- `did:wk:z6MkwFK7L2unwCxXNaws5wxbWKbzEiC84mCYno7RW5dZ7rzq@steve.monocore.dev/pub`
- `did:wk:z6MkwFK7L2unwCxXNaws5wxbWKbzEiC84mCYno7RW5dZ7rzq@steve.monocore.dev`
- `did:wk:z6MkwFK7L2unwCxXNaws5wxbWKbzEiC84mCYno7RW5dZ7rzq`

**Explanation:**

- If a locator component is not present, the `did:wk` functions similarly to a `did:key` [^1]. The core identifier is derived from the public key, enabling basic verification.
- If the locator component is provided, it points to a hosted DID document. The document, located at the well-known path (`https://<locator-component>/.well-known/did.json`), contains further metadata, authentication methods, service endpoints, and a mandatory `proof` section cryptographically linking the DID document to the public key.

## 3. Operations

### 3.1 Create

This section outlines the steps involved in creating a `did:wk` DID, generating the associated cryptographic material, and optionally, the initial DID document structure.

#### 3.1.1. Document Creation

1. **Generate Key Pair:** Choose a supported cryptographic algorithm (e.g., Ed25519, Secp256k1). Generate a new key pair for this DID.

2. **Determining DID Document Mode:**

   - **No Locator Component:** If the DID does not include a locator component (`@<locator-component>`), the DID and DID document are determined as per the `did:key` specification.
   - **Locator Component Present:** Proceed to steps 3 and 4 to create and fetch a hosted DID document.

3. **DID Document Template (locator mode):**

```json5
{
  "@context": "https://www.w3.org/ns/did/v1",
  "id": "did:wk:<encoded-public-key>[@<locator-component>]",
  "verificationMethod": [
    {
      "id": "did:wk:<encoded-public-key>[@<locator-component>]#keys-1",
      "type": "Ed25519VerificationKey2018", // Or other supported type
      "controller": "did:wk:<encoded-public-key>[@<locator-component>]",
      "publicKeyBase58": "<base58-encoded-public-key>"
    }
  ],
  "authentication": ["did:wk:<encoded-public-key>[@<locator-component>]#keys-1"],
  "proof": {
    // Signature details to be added later
  }
}
```

4. **Replace Placeholders:**

   - **`<encoded-public-key>`:** Replace with the encoded public key generated in step 1 (See section 3.1.4).
   - **`<locator-component>`:** Replace these with the corresponding URL segments for the planned location of the DID document.

5. **Proof Creation:** Proceed to section 3.1.2.

#### 3.1.2 Signature Method Creation

1. **Choose Signing Algorithm:** Select a supported signature algorithm for the `proof` section.

2. **Populate "proof" Section:**

- `type`: Set to the chosen signature type (e.g., `"Ed25519Signature2018"`).
- `created`: Set to a current timestamp in ISO 8601 format.
- `verificationMethod`: Set to the `id` of the verification method in the DID document associated with the signing key (likely `did:wk:<encoded-public-key>[@<locator-component>]]#keys-1`).
- `proofPurpose`: Set to `"assertionMethod"`.
- `nonce`: Generate a sufficiently random nonce to prevent replay attacks. This field is optional but generally recommended.

3. **Generate Signature:**

- **Serialize Document:** Canonicalize the DID document, excluding the `proof` section itself, according to the chosen serialization format.
- **Sign:** Sign the serialized document using the private key corresponding to the public key in the `verificationMethod`.
- **Base64 Encode:** Encode the signature bytes using Base64.
- **Set `signatureValue`:** Set the `signatureValue` field in the `proof` section to the encoded signature.

#### 3.1.3. Decode Public Key

1. **Extract Encoded Key:** Identify the portion of the `did:wk` method-specific identifier following the `did:wk:` scheme and up to the optional `@` symbol.
2. **Multibase Decoding:** Decode the extracted portion using the specified multibase encoding (e.g., base58btc).
3. **Multicodec Decoding:** Decode the raw bytes from step 2 according to the multicodec prefix, yielding the raw public key bytes in their original format.

#### 3.1.4. Encode Public Key

1. **Multicodec Encoding:**

- Choose a multicodec identifier based on the public key type (e.g., `ed25519-pub`).
- Encode the raw public key bytes according to the multicodec specification.

2. **Multibase Encoding:** Encode the multicodec output using the desired multibase format (e.g., `base58btc`).

## 3.2 Read

1. **Resolve DID:**

   - **No Locator Component:** Resolve the DID according to the `did:key` specification. The DID document itself is derived from the public key portion of the DID.
   - **Locator Component Present**
     - **Fetch:** Retrieve the DID document from the final URL (`https://<locator-component>/.well-known/did.json`).
     - **Verify Signature:**
       - Locate the `proof` section within the document.
       - Ensure the `verificationMethod` matches a key controlled by the entity resolving the DID.
       - Validate the created timestamp and nonce.
       - Verify the signature against the serialized DID document (excluding the `proof` section).

2. **Utilizing the DID Document:** Once resolved and validated (if applicable), the DID document is used as per the standard DID specifications (authentication, service endpoints, etc.).

## 3.3 Update

Updates primarily apply when a locator component is present and the corresponding DID document is hosted.

1. **Key Rotation:**

   - Generate a new key pair.
   - Modify the DID Document:
     - Add a new `verificationMethod` section for the new key.
     - Optionally, revoke (or mark as superseded) the old key in the appropriate DID document sections.
     - Update the `proof` section with a new signature generated using the new signing key.
   - Upload the modified DID document to the specified host/path.

2. **Service Endpoint Changes:**

   - Modify the relevant sections in the DID document.
   - Update the `proof` with a new signature.
   - Upload the modified DID document to the specified host/path.

## 3.4 Deactivate

Deactivation has different implications depending on whether the DID functions in 'key mode' or has a resolvable URL.

- **No Locator Component (did:key mode):** Deactivation is implied by the loss or revocation of the private key associated with the DID. There is no hosted DID document to modify in this scenario.

- **Locator Component Present:**

  - **Option 1: Removal** Delete the DID document from the hosted location. Resolution will fail.
  - **Option 2: Revocation Metadata** Add a 'revoked' status or similar mechanism within the DID document itself, update the `proof` with a new signature, and upload the modified document.

**Considerations**

- **Revocation Detail:** The specific mechanisms of revocation (how it's represented in the DID document) would likely need its own section in your proposal if you envision complex revocation scenarios.
- **Authorization:** If DID document updates are to be protected, you'll need to outline an authorization mechanism for controlling who is permitted to make changes.

## 4. Security & Privacy Considerations

### 4.1. Key Rotation

- **Regular Rotation:** Proactive key rotation is a best practice. Consider rotating keys periodically (e.g., annually) or in response to suspected compromise.
- **Rotation Procedure:**
  1. Generate a new key pair.
  2. Update the DID document:
     - Add the new key to `verificationMethod`.
     - Optionally, mark the old key as superseded or remove it (depending on revocation policy).
     - Create a new signature in the `proof` section.
  3. Upload the modified DID document (if a locator component is present).

### 4.2. Key Revocation

- **Revocation Mechanisms:**
  - **Status Flag:** Add a `revoked: true` field to the relevant `verificationMethod` section in the DID document.
  - **Key Removal:** Fully remove the compromised key from the `verificationMethod` section. Optionally include a reason for revocation.
- **Propagation:** Implement strategies to notify parties that may have the DID cached:
  - Versioning within the DID document.
  - Utilizing external revocation lists, if applicable.

### 4.3. Caveats

- **Domain Security:** While the `proof` mechanism within `did:wk` DID documents mitigates some risks associated with domain ownership, secure domain management (strong passwords, 2FA, reputable registrars) remains good practice for overall security and to avoid potential disruptions due to expiration or non-renewal.
- **Availability:** If a DID document is hosted, outages or disruptions could affect DID resolution. Consider redundancy (multiple hosts) or local caching mechanisms to improve resilience.
- **Signature Freshness:** The `proof` section's timestamps and nonce prevent replay attacks. Verifiers should reject signatures exceeding a specified maximum age.

[^1]: https://w3c-ccg.github.io/did-method-key/
[^2]: https://w3c-ccg.github.io/did-method-web/
[^3]: https://tools.ietf.org/html/rfc3986
[^4]: https://multiformats.io/

[steve-github]: https://github.com/appcypher
