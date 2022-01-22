const WebSocket = require('ws')
const url = 'ws://localhost:8000/lupus'
const connection = new WebSocket(url)
const prompt = require('prompt')

function get_prompt() {
	prompt.get(command, function(E, R) {
		if (E) {
			return onErr(E);
		}

		console.log("sending ", R)
		
		connection.onopen = () => {
			connection.send(R) 
		}

		connection.onerror = (error) => {
			console.log("WebSocket error: ", error)
		}

		connection.onmessage = (e) => {
			console.log(e.data)
		}

		get_prompt()
	})

	function onErr(e) {
		console.log(e);
	}
}

let command = {
	properties: {
		cmd: {
			message: 'enter a command',
			required: true
		}	
	}
}

prompt.start()
get_prompt()
