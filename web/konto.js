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
        "Meine Abonnements",
        "Einstellungen",
    ]
} else if (kontotyp == "bearbeiter") {
    sidebar_items = [
        "Meine Änderungen",
        "Meine Grundbuchblätter",
        "Meine Abonnements",
        "Einstellungen",
    ]
}

var active_sidebar = 0;
var filter_by = null;
var sort_by = null;
var selected = [];

function getActiveSectionName() {
    if (kontotyp == "admin") {
        if (active_sidebar == 0) {
            return "aenderungen";
        } else if (active_sidebar == 1) {
            return "zugriffe";
        } else if (active_sidebar == 2) {
            return "benutzer";
        } else if (active_sidebar == 3) {
            return "bezirke";
        } else if (active_sidebar == 4) {
            return "meine-kontodaten";
        } else {
            return "";
        }
    } else {
        return "";
    }
}

function updateFilter(target) {
    if (!target) {
        return;
    }
    filter_by = target.value;
    var active_section = getActiveSectionName();
    var node_data = document.getElementById("main-table-data");
    node_data.innerHTML = '';
    node_data.appendChild(renderRows(active_section));
}

function changeSection(target) {
    active_sidebar = target.dataset.index;
    renderSidebar();
    renderMainTable();
}

function addToSelection(target) {

}

function removeFromSelection(target) {

}

function selectAllVisible() {

}

function deselectAll() {
    selected = [];
    renderHeader(active_sidebar)
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
    check_uncheck_all_node_div.style.flexGrow = "0";
    check_uncheck_all_node_div.style.maxWidth = "18px";
    check_uncheck_all_node_div.style.minWidth = "18px";
    check_uncheck_all_node_div.style.borderBottom = "2px solid grey";
    var check_uncheck_all_node = document.createElement("input");
    check_uncheck_all_node.type = "checkbox";
    check_uncheck_all_node.style.minWidth = "15px";
    check_uncheck_all_node_div.appendChild(check_uncheck_all_node);
    header_column_node.appendChild(check_uncheck_all_node_div);

    var non_check_node = document.createElement("div");
    non_check_node.style.display = "flex";
    non_check_node.style.flexGrow = "1";
    for (var i = 0; i < spalten.length; i++) {
        var element = spalten[i];
        var cell_node = document.createElement("p");
        cell_node.style.minWidth = (100 / spalten.length) + "%";
        cell_node.style.maxWidth = (100 / spalten.length) + "%";
        var textnode = document.createTextNode(element);
        cell_node.appendChild(textnode);
        non_check_node.appendChild(cell_node);
    }
    header_column_node.appendChild(non_check_node);
    return header_column_node;
}

function renderSidebar() {

    document.getElementById("sidebar").innerHTML = '';

    for (var index = 0; index < sidebar_items.length; index++) {
        var element = sidebar_items[index];

        var node = document.createElement("a");
        node.style.display = "flex";
        node.tabIndex = "1";
        node.style.cursor = "pointer";
        node.style.width = "100%";
        node.style.textDecoration = "underline";
        if (active_sidebar == index) {
            node.style.color = "rgb(185, 14, 14)";
        }
        node.dataset.index = index;
        node.onclick = function(){ changeSection(this) };
        node.onkeyup = function(e){ if (e.key == "Enter") { changeSection(this) } };

        var textnode = document.createTextNode(element);
        node.appendChild(textnode);
        document.getElementById("sidebar").appendChild(node);    
    }
}

function rowIsValid(cells, filter) {
    if (!filter) {
        return true;
    }
    for (var i = 0; i < cells.length; i++) {
        var e = cells[i];
        if (e.includes(filter)) { return true; }
    }
    return false;
}

function renderRows(id) {
    var node_data = document.createElement("div");
    var kontoDaten = getKontoDaten();
    var keys = Object.keys(kontoDaten.data[id].daten);
    // sort_by(keys, filter_by)
    for (var i = 0; i < keys.length; i++) {
        var e = keys[i];
        var row = kontoDaten.data[id].daten[e];
        if (!rowIsValid(row, filter_by)) { continue; }
        var row_node = document.createElement("div");
        row_node.dataset.index = e;

        if (kontotyp == "admin" && id == "aenderungen") {

            var aenderung_id = row[1];
            var aenderung_name = row[1];
            var aenderung_email = row[2];
            var titel = row[6];
            var beschreibung = row[7];

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            if (selected.includes(aenderung_id)) {
                check_node.checked = true;
            }
            check_uncheck_all_node_div.appendChild(check_node);
            row_node.appendChild(check_uncheck_all_node_div);

            var non_check_node = document.createElement("div");
            non_check_node.style.display = "flex";
            non_check_node.style.flexGrow = "1";
            non_check_node.style.flexDirection = "row";

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "50%";
            cell_node.style.minWidth = "50%";
            cell_node.style.maxWidth = "50%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(aenderung_name);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(aenderung_email);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "50%";
            cell_node.style.minWidth = "50%";
            cell_node.style.maxWidth = "50%";

            var cell_text = document.createElement("p");
            cell_node.classList.add("aenderung-titel");
            var textnode1 = document.createTextNode(titel);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            var cell_text = document.createElement("p");
            cell_node.classList.add("aenderung-beschreibung");
            var textnode1 = document.createTextNode(beschreibung);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);
            
            row_node.appendChild(non_check_node);
        } else if (kontotyp == "admin" && id == "zugriffe") {

            var zugriff_id = row[0];
            var zugriff_name = row[1];
            var zugriff_email = row[2];
            var zugriff_typ = row[3];
            var zugriff_grund = row[4];

            var zugriff_land = row[5];
            var zugriff_amtsgericht = row[6];
            var zugriff_bezirk = row[7];
            var zugriff_blatt = row[8];

            var angefragt = row[9];
            var gewaehrt_von = row[10];
            var abgelehnt_von = row[11];
            var am = row[12];

            var options = { year: '2-digit', month: '2-digit', day: '2-digit', hour: "2-digit", minute: "2-digit", second: "2-digit" };
            var angefragt_date = new Date(Date.parse(angefragt)).toLocaleDateString("de-DE", options);

            var status = "Warte auf Zugriff, angefragt am";
            var line2 = angefragt_date;

            if (angefragt != "" && gewaehrt_von != "") {
                var am = new Date(Date.parse(am)).toLocaleDateString("de-DE", options);
                status = "Gewährt von" + gewaehrt_von;
                line2 = "am " + am;
            } else if (angefragt != "" && abgelehnt_von != "") {
                var am = new Date(Date.parse(am)).toLocaleDateString("de-DE", options);
                status = "Abgelehnt von" + abgelehnt_von;
                line2 = "am " + am;
            }

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            if (selected.includes(zugriff_id)) {
                check_node.checked = true;
            }
            check_uncheck_all_node_div.appendChild(check_node);
            row_node.appendChild(check_uncheck_all_node_div);


            var non_check_node = document.createElement("div");
            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "25%";
            cell_node.style.minWidth = "25%";
            cell_node.style.maxWidth = "25%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(zugriff_name);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(zugriff_email);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "25%";
            cell_node.style.minWidth = "25%";
            cell_node.style.maxWidth = "25%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(zugriff_typ.replaceAll("\"", ""));
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(zugriff_grund);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "25%";
            cell_node.style.minWidth = "25%";
            cell_node.style.maxWidth = "25%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(zugriff_land);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(zugriff_amtsgericht);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(zugriff_bezirk);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(zugriff_blatt);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);


            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "25%";
            cell_node.style.minWidth = "25%";
            cell_node.style.maxWidth = "25%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(status);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(line2);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);
            row_node.appendChild(non_check_node);

        } else if (kontotyp == "admin" && id == "benutzer") {

            var benutzer_name = row[0]; 
            var benutzer_email = row[1]; 
            var benutzer_rechte = row[2]; 
            var pubkey_fingerprint = row[3]; 
            var pubkey = row[4]; 

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            if (selected.includes(benutzer_email)) {
                check_node.checked = true;
            }
            check_uncheck_all_node_div.appendChild(check_node);
            row_node.appendChild(check_uncheck_all_node_div);


            var non_check_node = document.createElement("div");

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "25%";
            cell_node.style.minWidth = "25%";
            cell_node.style.maxWidth = "25%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(benutzer_name);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "25%";
            cell_node.style.minWidth = "25%";
            cell_node.style.maxWidth = "25%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(benutzer_email);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "25%";
            cell_node.style.minWidth = "25%";
            cell_node.style.maxWidth = "25%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(benutzer_rechte);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "25%";
            cell_node.style.minWidth = "25%";
            cell_node.style.maxWidth = "25%";

            var cell_text = document.createElement("button");
            cell_text.textContent = "Öffentlicher Schlüssel";
            if (pubkey == "") {
                cell_text.textContent = "Schlüsselpaar generieren";
            }
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            row_node.appendChild(non_check_node);

        } else if (kontotyp == "admin" && id == "bezirke") {
            
            var land = row[0];
            var amtsgericht = row[1];
            var bezirk = row[2];

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            if (selected.includes("" + i)) {
                check_node.checked = true;
            }
            check_uncheck_all_node_div.appendChild(check_node);
            row_node.appendChild(check_uncheck_all_node_div);

            var non_check_node = document.createElement("div");

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "33.3%";
            cell_node.style.minWidth = "33.3%";
            cell_node.style.maxWidth = "33.3%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(land);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "33.3%";
            cell_node.style.minWidth = "33.3%";
            cell_node.style.maxWidth = "33.3%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(amtsgericht);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "33.3%";
            cell_node.style.minWidth = "33.3%";
            cell_node.style.maxWidth = "33.3%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(bezirk);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            row_node.appendChild(non_check_node);
        } else if (kontotyp == "admin" && id == "meine-kontodaten") {
            
            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            check_uncheck_all_node_div.appendChild(check_node);
            row_node.appendChild(check_uncheck_all_node_div);

            var non_check_node = document.createElement("div");
            row_node.appendChild(non_check_node);
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
        change.textContent = "Neuen Benutzer anlegen";
        change.onclick = function(){ benutzerNeu(this) };
        actions_data.appendChild(change);
    
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

    var active_section = getActiveSectionName();
    node_header.appendChild(renderHeader(active_section));
    node_data.appendChild(renderRows(active_section));
    node_actions.appendChild(renderActions(active_section));
}

function benutzerNeu(target) {

}

function renderActionsBar() {

}

renderSidebar();
renderMainTable();
document.getElementById("main-table-filter").onchange = function() { updateFilter(this); }
document.getElementById("main-table-filter").oninput = function() { updateFilter(this); }