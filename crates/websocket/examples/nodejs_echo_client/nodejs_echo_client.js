#!/usr/bin/env node

const repl = require('repl')
const Websocket = require('isomorphic-ws')
const msgpack = require('msgpack-lite')

class HolochainWebsocket {
  constructor (socket) {
    this.socket = socket
  }

  static connect (url, messageCb) {
    return new Promise((resolve, reject) => {
      const socket = new Websocket(url)
      socket.onopen = () => {
        resolve(new HolochainWebsocket(socket))
      }
      socket.onmessage = data => {
        data = msgpack.decode(data.data)
        if (data.type === 'Signal') {
          messageCb({ type: 'signal', data: msgpack.decode(data.data) })
        } else if (data.type === 'Request') {
          messageCb({ type: 'request', id: data.id, data: msgpack.decode(data.data) })
        } else if (data.type === 'Response') {
          messageCb({ type: 'response', id: data.id, data: msgpack.decode(data.data) })
        }
      }
    })
  }

  signal (data) {
    data = msgpack.encode(data)
    data = msgpack.encode({
      type: 'Signal',
      data,
    })
    this.socket.send(data)
  }
}

async function main() {
  const r = repl.start({
    prompt: 'nodejs_echo_client> ',
    input: process.stdin,
    output: process.stdout,
    eval: async line => {
      connection.signal(line.trim())
      r.displayPrompt(true)
    },
  })

  const connection = await HolochainWebsocket.connect(
    'ws://127.0.0.1:12345',
    message => {
      console.log('\nReceived: ' + message.data)
      r.displayPrompt(true)
    }
  )

  r.on('exit', () => {
    process.exit(0)
  })
}

main().then(() => {}, err => {
  console.error(err)
  process.exit(1)
})
