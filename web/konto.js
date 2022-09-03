'use strict';

function getKontoDaten() {
    return JSON.parse(document.getElementById("konto-daten").dataset.kontoDaten);
}

function setKontoDaten(daten) {
    document.getElementById("konto-daten").dataset.kontoDaten = JSON.stringify(daten);
}

var kontotyp = getKontoDaten().kontotyp;
var sidebar_items = [];
if (kontotyp == "admin") {
    sidebar_items = [
        "Änderungen",
        "Zugriffe",
        "Benutzer",
        "Bezirke",
        "Einstellungen",
    ]
} else if (kontotyp == "gast") {
    sidebar_items = [
        "Meine Grundbuchblätter",
        "Einstellungen",
    ]
} else if (kontotyp == "bearbeiter") {
    sidebar_items = [
        "Meine Änderungen",
        "Meine Grundbuchblätter",
        "Einstellungen",
    ]
}

var active_sidebar = 0;
var filter_by = null;
var sort_by = null;

function updateFilter(event) {
    var input = document.getElementById("main-table-filter");
    if (!input) {
        return;
    }
}

function changeSection(target) {
    active_sidebar = target.dataset.index;
    renderSidebar();
    renderMainTable();
}

function renderHeader(id) {
    var kontoDaten = getKontoDaten();
    var header_column_node = document.createElement("div");
    for (var i = 0; i < kontoDaten.data[id].spalten.length; i++) {
        var element = kontoDaten.data[id].spalten[i];
        var cell_node = document.createElement("p");
        var textnode = document.createTextNode(element);
        cell_node.appendChild(textnode);
        header_column_node.appendChild(cell_node);
    }
    return header_column_node;
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

function renderRows(id) {
    var node_data = document.createElement("div");
    var kontoDaten = getKontoDaten();
    var keys = Object.keys(kontoDaten.data[id].daten);
    // sort_by(keys, filter_by)
    for (var i = 0; i < keys.length; i++) {
        var e = keys[i];
        // if !filter(e) { continue; }
        var row_node = document.createElement("div");
        row_node.dataset.index = e;

        var row = kontoDaten.data[id].daten[e];
        for (var k = 0; k < row.length; k++) {
            var cell = row[k];
            var cell_node = document.createElement("p");
            var textnode = document.createTextNode(cell);
            cell_node.appendChild(textnode);
            row_node.appendChild(cell_node);
        }

        node_data.appendChild(row_node);
    }
    return node_data;
}

function renderMainTable() {

    var node_actions = document.getElementById("main-table-actions");
    var node_data = document.getElementById("main-table-data");
    var node_header = document.getElementById("main-table-header");

    node_actions.innerHTML = '';
    node_data.innerHTML = '';
    node_header.innerHTML = '';

    var kontoDaten = getKontoDaten();

    if (kontoDaten.kontotyp == "admin") {
        if (active_sidebar == 0) {
            node_header.appendChild(renderHeader("aenderungen"));
            node_data.appendChild(renderRows("aenderungen"));
        } else if (active_sidebar == 1) {
            node_header.appendChild(renderHeader("zugriffe"));
            node_data.appendChild(renderRows("zugriffe"));
        } else if (active_sidebar == 2) {
            node_header.appendChild(renderHeader("benutzer"));
            node_data.appendChild(renderRows("benutzer"));
        } else if (active_sidebar == 3) {
            node_header.appendChild(renderHeader("bezirke"));
            node_data.appendChild(renderRows("bezirke"));
        } else if (active_sidebar == 4) {
            // var keys = kontoDaten.data["meine-kontodaten"].daten.keys();

        }
    }
}

function renderActionsBar() {

}

renderSidebar();
renderMainTable();