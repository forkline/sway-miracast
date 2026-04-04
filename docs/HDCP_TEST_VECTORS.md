# HDCP 2.x Test Vectors and Known-Good Values

This document provides test vectors and known-good values for HDCP 2.x cryptographic operations, specifically for Kd (derived key) and H_prime derivation.

## Overview

HDCP 2.x uses the following cryptographic operations:
- **AES-128-CTR** for Kd derivation
- **HMAC-SHA256** for H_prime and L_prime derivation
- **RSA-2048** for Km encryption during AKE

## Test Vector from swaybeam Implementation

### Test Vector 1: Kd Derivation

From `/home/agil/swaybeam/crates/daemon/src/lib.rs` (test_aes_ctr_kd_derivation)

#### Inputs
- **r_tx** (64-bit nonce from transmitter):
  ```
  35c723c8f919be44
  ```
  Hex bytes: `[0x35, 0xc7, 0x23, 0xc8, 0xf9, 0x19, 0xbe, 0x44]`

- **Km** (128-bit master key from RSA decryption):
  ```
  089c19d2391586f016055a213952d762
  ```
  Hex bytes: `[0x08, 0x9c, 0x19, 0xd2, 0x39, 0x15, 0x86, 0xf0, 0x16, 0x05, 0x5a, 0x21, 0x39, 0x52, 0xd7, 0x62]`

#### Expected Output
- **Kd** (256-bit derived key):
  ```
  e9da8dc5f71ab59aab9839c28d26ab6a5283a2a6db01713109424514d67fe913
  ```

#### Algorithm Details

Kd is derived using AES-128-CTR mode:

1. **First Block (Kd[0:16])**:
   - IV: `r_tx` in bytes 0-7, zeros in bytes 8-15, counter 0 in byte 15
   - IV hex: `35c723c8f919be44 0000000000000000`
   - AES-128-CTR encrypt zeros with Km as key

2. **Second Block (Kd[16:32])**:
   - IV: Same as first block but counter 1 in byte 15
   - IV hex: `35c723c8f919be44 0000000000000001`
   - AES-128-CTR encrypt zeros with Km as key

3. **Concatenate**: Kd = FirstBlock + SecondBlock

### Test Vector 2: H_prime Derivation

H_prime is derived from Kd using HMAC-SHA256:

#### Inputs
- **Key**: Kd (256-bit derived key from above)
- **Message**: r_tx (64-bit nonce)

#### Expected Output
- **H_prime** (256-bit hash):
  ```
  HMAC-SHA256(Kd, r_tx)
  ```

#### Algorithm
```
H_prime = HMAC-SHA256(Kd, r_tx)
```

Where:
- Kd is the 256-bit derived key
- r_tx is the 64-bit nonce from transmitter
- HMAC uses SHA-256 as the hash function

### Test Vector 3: L_prime Derivation

L_prime is used for locality check and is derived as follows:

#### Inputs
- **Kd**: 256-bit derived key
- **r_rx**: 64-bit nonce from receiver
- **r_n**: 64-bit nonce from transmitter (generated for locality check)

#### Algorithm

1. XOR Kd bytes [24:32] with r_rx:
   ```
   key = Kd
   key[24:32] ^= r_rx
   ```

2. Compute HMAC:
   ```
   L_prime = HMAC-SHA256(key, r_n)
   ```

## Implementation in swaybeam

The swaybeam project implements HDCP 2.x cryptographic operations in:

- **File**: `crates/daemon/src/lib.rs`
- **Functions**:
  - `compute_hdcp_kd()` - AES-128-CTR Kd derivation
  - `compute_hdcp_h_prime()` - HMAC-SHA256 H_prime computation
  - `compute_hdcp_l_prime()` - HMAC-SHA256 L_prime computation
  - `compute_hmac_sha256()` - HMAC-SHA256 implementation
  - `compute_hdcp_ctr_block()` - AES-128-CTR block encryption

## Verification Tools

### Running Tests

To verify the implementation:

```bash
cd /home/agil/swaybeam
cargo test test_aes_ctr_kd_derivation -- --nocapture
```

Expected output:
```
Kd: e9da8dc5f71ab59aab9839c28d26ab6a5283a2a6db01713109424514d67fe913
test tests::test_aes_ctr_kd_derivation ... ok
```

### Python Verification Script

You can create a Python script to verify these values:

```python
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives import hashes, hmac
from cryptography.hazmat.backends import default_backend

def compute_hdcp_kd(r_tx: bytes, km: bytes) -> bytes:
    """Compute HDCP Kd using AES-128-CTR"""
    assert len(r_tx) == 8
    assert len(km) == 16

    # First block
    iv1 = r_tx + bytes(8)
    cipher = Cipher(algorithms.AES(km), modes.CTR(iv1), backend=default_backend())
    encryptor = cipher.encryptor()
    block1 = encryptor.update(bytes(16)) + encryptor.finalize()

    # Second block (counter = 1)
    iv2 = r_tx + bytes(7) + bytes([1])
    cipher = Cipher(algorithms.AES(km), modes.CTR(iv2), backend=default_backend())
    encryptor = cipher.encryptor()
    block2 = encryptor.update(bytes(16)) + encryptor.finalize()

    return block1 + block2

def compute_hdcp_h_prime(kd: bytes, r_tx: bytes) -> bytes:
    """Compute HDCP H_prime using HMAC-SHA256"""
    h = hmac.HMAC(kd, hashes.SHA256(), backend=default_backend())
    h.update(r_tx)
    return h.finalize()

# Test vector
r_tx = bytes.fromhex('35c723c8f919be44')
km = bytes.fromhex('089c19d2391586f016055a213952d762')

# Compute Kd
kd = compute_hdcp_kd(r_tx, km)
print(f"Kd: {kd.hex()}")

# Expected Kd
expected_kd = 'e9da8dc5f71ab59aab9839c28d26ab6a5283a2a6db01713109424514d67fe913'
assert kd.hex() == expected_kd, f"Kd mismatch: got {kd.hex()}, expected {expected_kd}"

# Compute H_prime
h_prime = compute_hdcp_h_prime(kd, r_tx)
print(f"H_prime: {h_prime.hex()}")

print("All tests passed!")
```

## Sources and References

### Official Documentation
- HDCP 2.3 specification (not publicly available, requires license from DCP LLC)
- Wi-Fi Display (Miracast) specification includes HDCP 2.x requirements

### Academic Papers
- Cryptographic analysis papers on HDCP 2.x protocol (search on IACR ePrint)

### Open Source Implementations
1. **swaybeam**: `/home/agil/swaybeam/crates/daemon/src/lib.rs`
   - Rust implementation of HDCP 2.x
   - Includes unit tests with known-good values

2. **Other Implementations** (search GitHub):
   - Look for: "HDCP 2.2 implementation"
   - Look for: "Miracast HDCP"
   - Note: Most implementations are proprietary or not publicly available

### Limitations on Test Vector Availability

**Important Note**: HDCP 2.x test vectors are limited because:

1. **Proprietary Protocol**: HDCP is a proprietary protocol owned by Digital Content Protection LLC (DCP LLC)
2. **Licensing Required**: Access to official test vectors requires HDCP adopter license
3. **Limited Public Information**: Most detailed cryptographic test vectors are not publicly published
4. **Security Considerations**: Full test suites may reveal implementation details that could aid attacks

## Additional Test Values Needed

For complete testing, you would need:
- Valid receiver certificate (RSA-2048 public key)
- Encrypted Km values (RSA-2048 ciphertext)
- r_rx values (receiver nonce)
- r_n values (locality check nonce)
- Expected H_prime values (computed from above)
- Expected L_prime values (computed from above)

## Generating Additional Test Vectors

To generate additional test vectors, you can:

1. Use the Rust implementation in swaybeam
2. Implement the algorithms using cryptographic libraries
3. Verify against known HDCP 2.x devices (requires physical hardware)

### Example: Creating Test Vectors Programmatically

```rust
// In Rust, you can create test vectors using the existing implementation
use swaybeam_daemon::Daemon;

fn generate_test_vector() {
    // Generate random r_tx and Km
    let r_tx: [u8; 8] = random_64bit();
    let km: [u8; 16] = random_128bit();

    // Compute Kd
    let kd = Daemon::compute_hdcp_kd(&r_tx, &km);

    // Compute H_prime
    let h_prime = Daemon::compute_hdcp_h_prime(&r_tx, &km);

    // Print test vector
    println!("r_tx: {}", hex::encode(&r_tx));
    println!("Km: {}", hex::encode(&km));
    println!("Kd: {}", hex::encode(&kd));
    println!("H_prime: {}", hex::encode(&h_prime));
}
```

## Summary

### Available Test Vectors

| Parameter | Value (Hex) |
|-----------|-------------|
| r_tx | 35c723c8f919be44 |
| Km | 089c19d2391586f016055a213952d762 |
| Kd | e9da8dc5f71ab59aab9839c28d26ab6a5283a2a6db01713109424514d67fe913 |

### Missing Test Vectors

- H_prime expected value (can be computed from above)
- L_prime expected value (requires r_rx and r_n)
- Valid receiver certificates
- RSA encryption test vectors

### Verification Commands

```bash
# Run existing test
cargo test test_aes_ctr_kd_derivation -- --nocapture

# View implementation
cat crates/daemon/src/lib.rs | grep -A 30 "fn compute_hdcp_kd"
```

## Conclusion

While official HDCP 2.x test vectors are not publicly available due to licensing restrictions, the swaybeam project provides a working implementation with verified test vectors for Kd derivation. Additional test vectors can be generated using the implementation, but verification against real HDCP-compliant devices requires physical hardware and proper licensing.
