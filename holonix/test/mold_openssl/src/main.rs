fn main() {
    use openssl::encrypt::{Decrypter, Encrypter};
    use openssl::pkey::PKey;
    use openssl::rsa::{Padding, Rsa};

    // Generate a keypair
    let keypair = Rsa::generate(2048).unwrap();
    let keypair = PKey::from_rsa(keypair).unwrap();

    let data = b"hello, world!";

    // Encrypt the data with RSA PKCS1
    let mut encrypter = Encrypter::new(&keypair).unwrap();
    encrypter.set_rsa_padding(Padding::PKCS1).unwrap();
    // Create an output buffer
    let buffer_len = encrypter.encrypt_len(data).unwrap();
    let mut encrypted = vec![0; buffer_len];
    // Encrypt and truncate the buffer
    let encrypted_len = encrypter.encrypt(data, &mut encrypted).unwrap();
    encrypted.truncate(encrypted_len);

    // Decrypt the data
    let mut decrypter = Decrypter::new(&keypair).unwrap();
    decrypter.set_rsa_padding(Padding::PKCS1).unwrap();
    // Create an output buffer
    let buffer_len = decrypter.decrypt_len(&encrypted).unwrap();
    let mut decrypted = vec![0; buffer_len];
    // Encrypt and truncate the buffer
    let decrypted_len = decrypter.decrypt(&encrypted, &mut decrypted).unwrap();
    decrypted.truncate(decrypted_len);
    assert_eq!(&*decrypted, data);

    println!("{decrypted:#?}");
}
