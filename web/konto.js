'use strict';

function submitJsonRequest() {
    var token = document.getElementById("token-id").dataset.tokenId;
    console.log(JSON.parse(document.getElementById("konto-daten").dataset.kontoDaten));
    console.log(token);
}