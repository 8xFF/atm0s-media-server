import { WHEPClient } from "./whep.js"

window.start = async () => {
    console.log("Will start");
    //Create peerconnection
	const pc = window.pc = new RTCPeerConnection();

	//Add recv only transceivers
	pc.addTransceiver("audio", { direction: 'recvonly' });
	pc.addTransceiver("video", { direction: 'recvonly' });

	pc.ontrack = (event) =>
	{
		console.log(event)
		if (event.track.kind == "video")
			document.querySelector("video").srcObject = event.streams[0];
	}

	//Create whep client
	const whep = new WHEPClient();

	const url = "/whep/endpoint";
	const token = document.getElementById("room-id").value;

	//Start viewing
	whep.view(pc, url, token);

	window.whep_instance = whep;
}

window.stop = async () => {
    if (window.whep_instance) {
        window.whep_instance.stop();
    }

    document.getElementById("video").srcObject = null;
}