const WebSocket = require('ws')
const url = 'ws://localhost:7500/lupus'
const connection = new WebSocket(url)

console.log("sending", "ping")

connection.onopen = () => {
	connection.send("PING") 
	connection.send("PING") 
	connection.send("PING") 
	connection.send("PING") 
}

connection.onerror = (error) => {
	console.log("WebSocket error: ", error)
}

connection.onmessage = (e) => {
	console.log(e.data)
}

