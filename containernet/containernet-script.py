#!/usr/bin/python3
"""
This is the most simple example to showcase Containernet.
"""
import time
from mininet.net import Containernet
from mininet.node import Controller
from mininet.cli import CLI
from mininet.link import TCLink
from mininet.log import info, setLogLevel
setLogLevel('info')

net = Containernet(controller=Controller)
info('*** Adding controller\n')
net.addController('c0')
info('*** Adding docker containers\n')
d1 = net.addDocker('d1', ip='10.0.0.251', dimage="hc-containernet-base")
d2 = net.addDocker('d2', ip='10.0.0.252', dimage="hc-containernet-base")
info('*** Adding switches\n')
s1 = net.addSwitch('s1')
s2 = net.addSwitch('s2')
info('*** Creating links\n')
net.addLink(d1, s1)
net.addLink(s1, s2, cls=TCLink, delay='100ms', bw=1)
net.addLink(s2, d2)
info('*** Starting network\n')
net.start()
info('*** Testing connectivity\n')
net.ping([d1, d2])
info('*** Testing connectivity\n')
d1.cmd('kitsune-p2p-proxy > proxy-output 2>&1 &')
time.sleep(1)
addr = d1.cmd('cat proxy-output')
info('!!! PROXY ADDR %s' % addr)
res = d2.cmd('proxy-cli %s' % addr)
info('ping on d2 - got\n%s' % res)
info('*** Running CLI\n')
#CLI(net)
info('*** Stopping network')
net.stop()
