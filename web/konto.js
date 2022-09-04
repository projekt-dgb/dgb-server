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
var selected = [];

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
    var spalten = [];
    if (kontotyp == "admin" && id == "aenderungen") {
        spalten = [
            "Name",
            "Datum / Beschreibung"
        ];
    } else if (kontotyp == "admin" && id == "zugriffe") {
        spalten = [
            "Name",
            "Grund / Typ",
            "Bezirk",
            "Status",
        ];
    } else if (kontotyp == "admin" && id == "benutzer") {
        spalten = [
            "Name",
            "E-Mail",
            "Rechte",
            "Schlüssel",
        ];
    } else if (kontotyp == "admin" && id == "bezirke") {
        spalten = [
            "Land",
            "Amtsgericht",
            "Bezirk",
        ];
    } else if (kontotyp == "admin" && id == "meine-kontodaten") {
        spalten = [
            "Einstellung",
            "Wert"
        ]
    }

    var header_column_node = document.createElement("div");
    var check_uncheck_all_node_div = document.createElement("div");
    check_uncheck_all_node_div.style.padding = "5px 10px";
    check_uncheck_all_node_div.style.borderBottom = "2px solid grey";
    var check_uncheck_all_node = document.createElement("input");
    check_uncheck_all_node.type = "checkbox";
    check_uncheck_all_node.style.minWidth = "15px";
    check_uncheck_all_node_div.appendChild(check_uncheck_all_node);
    header_column_node.appendChild(check_uncheck_all_node_div);

    for (var i = 0; i < spalten.length; i++) {
        var element = spalten[i];
        var cell_node = document.createElement("p");
        var textnode = document.createTextNode(element);
        cell_node.appendChild(textnode);
        header_column_node.appendChild(cell_node);
    }
    return header_column_node;
}

function renderSidebar() {

    document.getElementById("sidebar").innerHTML = '';

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

function filterRow(e, filter) {
    if (!filter) {
        return false;
    }
    return e.includes(filter);
}

function renderRows(id) {
    var node_data = document.createElement("div");
    var kontoDaten = getKontoDaten();
    var keys = Object.keys(kontoDaten.data[id].daten);
    // sort_by(keys, filter_by)
    for (var i = 0; i < keys.length; i++) {
        var e = keys[i];
        if (!filterRow(e, filter_by)) { continue; }
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

function renderActions(id) {
    var actions_data = document.createElement("div");
    if (kontotyp == "admin" && id == "aenderungen") {

    } else if (kontotyp == "admin" && id == "zugriffe") {
        var genehmigen = document.createElement("button");
        genehmigen.textContent = "Zugriff genehmigen";
        actions_data.appendChild(genehmigen);

        var ablehnen = document.createElement("button");
        ablehnen.textContent = "Zugriff ablehnen";
        actions_data.appendChild(ablehnen);

        var zurueckziehen = document.createElement("button");
        zurueckziehen.textContent = "Zugriff zurückziehen";
        actions_data.appendChild(zurueckziehen);
    } else if (kontotyp == "admin" && id == "benutzer") {

        var change = document.createElement("button");
        change.textContent = "Benutzer bearbeiten";
        actions_data.appendChild(change);

        var loeschen = document.createElement("button");
        loeschen.textContent = "Benutzer löschen";
        actions_data.appendChild(loeschen);
    } else if (kontotyp == "admin" && id == "bezirke") {

        var bezirk_new = document.createElement("button");
        bezirk_new.textContent = "Bezirk hinzufügen";
        actions_data.appendChild(bezirk_new);

        var loeschen = document.createElement("button");
        loeschen.textContent = "Bezirk löschen";
        actions_data.appendChild(loeschen);
    }
    
    return actions_data;
}

function renderMainTable() {

    var node_actions = document.getElementById("main-table-actions");
    var node_data = document.getElementById("main-table-data");
    var node_header = document.getElementById("main-table-header");

    node_actions.innerHTML = '';
    node_data.innerHTML = '';
    node_header.innerHTML = '';

    if (kontotyp == "admin") {
        if (active_sidebar == 0) {
            node_header.appendChild(renderHeader("aenderungen"));
            node_data.appendChild(renderRows("aenderungen"));
            node_actions.appendChild(renderActions("aenderungen"));
        } else if (active_sidebar == 1) {
            node_header.appendChild(renderHeader("zugriffe"));
            node_data.appendChild(renderRows("zugriffe"));
            node_actions.appendChild(renderActions("zugriffe"));
        } else if (active_sidebar == 2) {
            node_header.appendChild(renderHeader("benutzer"));
            node_data.appendChild(renderRows("benutzer"));
            node_actions.appendChild(renderActions("benutzer"));
        } else if (active_sidebar == 3) {
            node_header.appendChild(renderHeader("bezirke"));
            node_data.appendChild(renderRows("bezirke"));
            node_actions.appendChild(renderActions("bezirke"));
        } else if (active_sidebar == 4) {
            // var keys = kontoDaten.data["meine-kontodaten"].daten.keys();
            node_header.appendChild(renderHeader("meine-kontodaten"));
            node_data.appendChild(renderRows("meine-kontodaten"));

        }
    }
}

function renderActionsBar() {

}

renderSidebar();
renderMainTable();