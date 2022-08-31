'use strict';

var sidebar_items = [
    "Änderungen",
    "Zugriffe",
    "Benutzer",
    "Bezirke",
    "Einstellungen",
];

var active_sidebar = 0;

function getKontoDaten() {
    return JSON.parse(document.getElementById("konto-daten").dataset.kontoDaten);
}

function setKontoDaten(daten) {
    document.getElementById("konto-daten").dataset.kontoDaten = JSON.stringify(daten);
}

function changeSection(target) {
    active_sidebar = target.dataset.index;
    console.log(active_sidebar);
    renderSidebar();
}

function renderSidebar() {

    document.getElementById("sidebar").innerHTML = '';

    if (getKontoDaten().kontotyp == "admin") {
        for (var index = 0; index < sidebar_items.length; index++) {
            var element = sidebar_items[index];

            var node = document.createElement("p");
            node.style.cursor = "pointer";
            node.style.width = "100%";
            node.style.textDecoration = "underline";
            if (active_sidebar == index) {
                node.style.color = "rgb(185, 14, 14)";
            }
            node.dataset.index = index;
            node.onclick = function(){ changeSection(this) };

            var textnode = document.createTextNode(element);
            node.appendChild(textnode);
            document.getElementById("sidebar").appendChild(node);    
        }
    }
}

renderSidebar();