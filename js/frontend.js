const WebSocket = require('ws')
const url = 'ws://192.168.1.95:12053/taurus'
const connection = new WebSocket(url)

console.log("sending", "ping")

connection.onopen = () => {
	connection.send("A") 
	connection.send("") 
	connection.send("test")
	connection.send("LIST_BACKUPS")
	connection.send("PING")
}

connection.onerror = (error) => {
	console.log("WebSocket error: ", error)
}

connection.onmessage = (e) => {
	console.log(e.data)
}

