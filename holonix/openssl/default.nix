{ perl, openssl }: {
  buildInputs = [
    # the OpenSSL static installation provided by native-tls rust module on linux
    # environments uses perl under the hood to configure and install the
    # statically linked openssl lib
    perl
  ];

  shellHook = ''
    # used by the OPENSSL_STATIC environment variable
    # when this is "1" the openssl crate on linux will locally build and
    # statically link the openssl lib
    # needs to be used in tandem with manifest config
    # @see holochain_net
    export static="1";

    export OPENSSL_STATIC=openssl.static;
  '';
}
