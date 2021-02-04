#!/usr/bin/python3
"""
This is the most simple example to showcase Containernet.
"""

import sys
import time
from mininet.topo import Topo
from mininet.net import Containernet
from mininet.node import Controller
from mininet.nodelib import NAT
from mininet.cli import CLI
from mininet.link import TCLink
from mininet.log import info, setLogLevel

def p(*args):
    "Print out some info with delimiter dashes"

    info('-----\n')
    info(*args, '\n')
    info('-----\n')

class Network:
    "Modularize some of the APIs to make it easier to work with"

    def __init__(self):
        "Constructor - set up our network topology"

        # setup the base containernet controller
        net = self.net = Containernet(controller=Controller)
        net.addController('c0')

        # our root backbone switch
        s0 = self.s0 = net.addSwitch('s0')

        # this host is publicly accessible
        h0 = self.h0 = net.addDocker(
            'h0',
            ip = '10.0.0.100',
            dimage = 'hc-containernet-base',
        )

        # connect to our backbone switch
        net.addLink(s0, h0)

        # create two nodes behind nats
        d1 = self.d1 = self.addNatNode(1)
        d2 = self.d2 = self.addNatNode(2)

        # start the simulation
        net.start()

    def __del__(self):
        "Destructor - just stop the network"

        self.stop()

    def stop(self):
        "Manually stop the network, called by __del__"

        try:
            self.net.stop()
        except:
            pass

    def addNatNode(self, idx):
        "Add a node behind a NAT to the network topology"

        # the interface that connects to the backbone switch
        inetIntf = 'nat%d-eth0' % idx

        # the interface that connects to the LAN host
        localIntf = 'nat%d-eth1' % idx

        # LAN ip
        localIP = '192.168.%d.1' % idx

        # LAN subnet
        localSubnet = '192.168.%d.0/24' % idx

        # LAN link params
        natParams = { 'ip' : '%s/24' % localIP }

        # create the NAT host - this is the LAN router
        nat = self.net.addHost(
            'nat%d' % idx,
            cls = NAT,
            subnet = localSubnet,
            inetIntf = inetIntf,
            localIntf = localIntf,
        )

        # create a LAN switch
        switch = self.net.addSwitch('s%d' %idx)

        # WAN link from router to backbone switch
        self.net.addLink(nat, self.s0, intfName1=inetIntf)

        # LAN link from router to LAN switch
        self.net.addLink(nat, switch, intfName1=localIntf, params1=natParams)

        # the local LAN host
        host = self.net.addDocker(
            'd%d' % idx,
            ip = '192.168.%d.100/24' % idx,
            dimage = 'hc-containernet-base',
            defaultRoute='via %s' % localIP,
        )

        # add a link to the LAN switch
        self.net.addLink(host, switch)

        # return the LAN host
        return host

def run():
    "Execute this test script"

    # output info level logs
    setLogLevel('info')

    # start up our network
    net = Network()

    def execPrint(node, cmd):
        "helper - prints a command and runs it printing the output"

        p('%s: %s' % (node.IP(), cmd))
        p(node.cmd(cmd))

    def exitCode(node, cmd):
        "helper - prints a command - runs it with no output - returns exit code"

        p('%s: %s' % (node.IP(), cmd))
        cmd = '%s > /dev/null 2>&1; echo $?' % cmd
        return int(node.cmd(cmd))

    # uncomment if you want to mess with the containernet REPL cli
    #CLI(net.net)

    # run our test script
    try:
        # make sure we can ping from LAN node 1 to public host
        res = exitCode(net.d1, 'ping -c1 %s' % net.h0.IP())
        p('GOT EXIT CODE: %d' % res)

        # make sure we can ping from LAN node 2 to public host
        res = exitCode(net.d2, 'ping -c1 %s' % net.h0.IP())
        p('GOT EXIT CODE: %d' % res)

        # make sure we CANNOT ping between LAN nodes
        res = exitCode(net.d1, 'ping -c1 %s' % net.d2.IP())
        p('GOT EXIT CODE: %d' % res)

        # Startup kitsune proxy server on the public host
        p('Starting Proxy Server')
        net.h0.cmd('kitsune-p2p-proxy --bind-to kitsune-quic://10.0.0.100:0 > proxy-output 2>&1 &')
        time.sleep(1)
        addr = net.h0.cmd('cat proxy-output').strip()
        p('PROXY ADDR: %s' % addr)

        # run the proxy-cli utility from LAN nodes to public proxy server
        execPrint(net.d1, 'proxy-cli %s' % addr)
        execPrint(net.d2, 'proxy-cli %s' % addr)
    except:
        p('ERROR: %s', sys.exc_info())

    # stop / cleanup the network simulation
    net.stop()

# entrypoint
if __name__ == '__main__':
    run()
