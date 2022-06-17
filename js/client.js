const WebSocket = require('ws')
const url = 'ws://192.168.1.120:11800/taurus'
const connection = new WebSocket(url)

console.log("sending", "ping")

connection.onopen = () => {
	console.log("s")
	connection.send("PING")
	//connection.send("TOGGLE_BRIDGE SAGCMP")
}

connection.onerror = (error) => {
	console.log("WebSocket error: ", error)
}

connection.onmessage = (e) => {
	console.log(e.data)
}

