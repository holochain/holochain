## containernet test setup for holochain network topology testing

### Requirements

- bash
- virtualbox - https://www.virtualbox.org/wiki/Linux_Downloads
- vagrant - https://www.vagrantup.com/downloads.html

### Execute

`./run-containernet.bash`

### The process

1. Builds an ubuntu bionic vagrant/virtualbox containernet VM
2. Builds docker container images containing holochain binaries
3. Runs the provided containernet-script.py test suite
4. Exits

The build is based off your working directory files... and while it cannot use your local cargo cache, it will keep a cache inside the virtualbox vm, so subsequent builds will be faster, and you can iterate locally.
