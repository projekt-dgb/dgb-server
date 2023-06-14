'use strict';

var downloadBlob = function(data, fileName, mimeType) {
  var blob, url;
  blob = new Blob([data], {
    type: mimeType
  });
  url = window.URL.createObjectURL(blob);
  downloadURL(url, fileName);
  setTimeout(function() {
    return window.URL.revokeObjectURL(url);
  }, 1000);
};

var downloadURL = function(data, fileName) {
  var a;
  a = document.createElement('a');
  a.href = data;
  a.download = fileName;
  document.body.appendChild(a);
  a.style = 'display: none';
  a.click();
  a.remove();
};

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
        "Grundbücher",
        "Änderungen",
        "Zugriffe",
        "Benutzer",
        "Bezirke",
        "Abonnements",
        "Einstellungen",
    ]
} else if (kontotyp == "bearbeiter") {
    sidebar_items = [
        "Grundbücher",
        "Änderungen",
        "Abonnements",
        "Einstellungen",
    ]
} else if (kontotyp == "gast") {
    sidebar_items = [
        "Grundbücher",
        "Abonnements",
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
            return "blaetter";
        } else if (active_sidebar == 1) {
            return "aenderungen";
        } else if (active_sidebar == 2) {
            return "zugriffe";
        } else if (active_sidebar == 3) {
            return "benutzer";
        } else if (active_sidebar == 4) {
            return "bezirke";
        } else if (active_sidebar == 5) {
            return "abonnements";
        } else if (active_sidebar == 6) {
            return "meine-kontodaten";
        } else {
            return "";
        }
    } else if (kontotyp == "gast") {
        if (active_sidebar == 0) {
            return "blaetter";
        } else if (active_sidebar == 1) {
            return "abonnements";
        } else if (active_sidebar == 2) {
            return "meine-kontodaten";
        } else {
            return "";
        }
    } else if (kontotyp == "bearbeiter") {
        if (active_sidebar == 0) {
            return "aenderungen";
        } else if (active_sidebar == 1) {
            return "blaetter";
        } else if (active_sidebar == 2) {
            return "abonnements";
        }  else if (active_sidebar == 3) {
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
    selected = [];
    filter_by = null;
    renderSidebar();
    renderMainTable();
    updateFilter();
}

function addToSelection(target) {
    if (!target) {
        return;
    }
    var id = target.dataset.id;
    selected.push(id);
    selected.sort();
    renderMainTable();
}

function removeFromSelection(target) {
    if (!target) {
        return;
    }

    var id = target.dataset.id;
    selected.sort();
    var newarray = [];
    for (var i = 0; i < selected.length; i++) {
        var element = selected[i];
        if (element != id) {
            newarray.push(element);
        }
    }
    selected = newarray;
    renderMainTable();
}

function selectAllVisible() {

    var active_section = getActiveSectionName();
    var kontoDaten = getKontoDaten();
    var activeSection = kontoDaten.data[active_section];
    if (!activeSection) {
        return;
    }
    var keys = Object.keys(activeSection.daten);
    selected = [];
    for (var i = 0; i < keys.length; i++) {
        var e = keys[i];
        var row = activeSection.daten[e];
        if (!rowIsValid(row, filter_by)) { continue; }
        if (kontotyp == "admin" && active_section == "aenderungen") {
            var aenderung_id = row[0];
            selected.push(aenderung_id);
        } else if (kontotyp == "admin" && active_section == "zugriffe") {
            var zugriff_id = row[0];
            selected.push(zugriff_id);
        } else if (kontotyp == "admin" && active_section == "benutzer") {
            var benutzer_email = row[1];
            selected.push(benutzer_email);
        } else if (kontotyp == "admin" && active_section == "bezirke") {
            var bezirk_id = row[0];
            selected.push(bezirk_id);
        } else if (kontotyp == "admin" && active_section == "meine-kontodaten") {
        
        } else if (active_section == "blaetter") {
            var blatt_id = row[0] + "/" + row[1] + "/" + row[2] + "/" + row[3];
            selected.push(blatt_id);
        }
    }
    selected.sort();
    renderMainTable();
}

function deselectAll() {
    selected = [];
    renderMainTable();
}

function renderHeader(id) {
    var spalten = [];
    if ((kontotyp == "admin" && id == "aenderungen") ||
        (kontotyp == "bearbeiter" && id == "aenderungen")) {
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
    } else if (
        (kontotyp == "admin" && id == "meine-kontodaten") || 
        (kontotyp == "bearbeiter" && id == "meine-kontodaten") ||
        (kontotyp == "gast" && id == "meine-kontodaten")
    ) {
        spalten = [
            "Einstellung",
            "Wert"
        ];
    } else if (id == "blaetter") {
        spalten = [
            "Land",
            "Amtsgericht",
            "Bezirk",
            "Blatt"
        ];
    } else if (id == "abonnements") {
        spalten = [
            "Typ",
            "Benutzer / URL",
            "Amtsgericht",
            "Bezirk",
            "Blatt",
            "Aktenzeichen",
        ];
    }

    var header_column_node = document.createElement("div");
    var check_uncheck_all_node_div = document.createElement("div");
    check_uncheck_all_node_div.style.flexDirection = "column";
    check_uncheck_all_node_div.style.padding = "5px 10px";
    check_uncheck_all_node_div.style.flexGrow = "0";
    check_uncheck_all_node_div.style.maxWidth = "18px";
    check_uncheck_all_node_div.style.minWidth = "18px";
    check_uncheck_all_node_div.style.borderBottom = "2px solid grey";
    var check_uncheck_all_node = document.createElement("input");
    check_uncheck_all_node.type = "checkbox";
    check_uncheck_all_node.checked = selected.length != 0;
    check_uncheck_all_node.addEventListener('change', function(event) {
        if (event.currentTarget.checked) {
            selectAllVisible();
        } else {
            deselectAll();
        }
    });
    check_uncheck_all_node.style.minWidth = "15px";
    check_uncheck_all_node_div.appendChild(check_uncheck_all_node);
    header_column_node.appendChild(check_uncheck_all_node_div);

    if (id == "blaetter") {
        var check_uncheck_all_node_div = document.createElement("div");
        check_uncheck_all_node_div.style.maxWidth = "18px";
        check_uncheck_all_node_div.style.minWidth = "18px";
        check_uncheck_all_node_div.style.borderBottom = "2px solid grey";
        header_column_node.appendChild(check_uncheck_all_node_div);    
    }

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

    document.getElementById("sidebar").appendChild(document.createElement("br"));    

    var node = document.createElement("a");
    node.style.display = "flex";
    node.tabIndex = "1";
    node.style.cursor = "pointer";
    node.style.width = "100%";
    node.style.textDecoration = "underline";
    node.onclick = function(){ benutzerAbmelden() };
    node.onkeyup = function(e){ if (e.key == "Enter") { benutzerAbmelden() } };

    var textnode = document.createTextNode("Abmelden");
    node.appendChild(textnode);
    document.getElementById("sidebar").appendChild(node);    
}

function benutzerAbmelden() {
    var cookies = document.cookie.split("; ");
    for (var c = 0; c < cookies.length; c++) {
        var d = window.location.hostname.split(".");
        while (d.length > 0) {
            var cookieBase = encodeURIComponent(cookies[c].split(";")[0].split("=")[0]) + '=; expires=Thu, 01-Jan-1970 00:00:01 GMT; domain=' + d.join('.') + ' ;path=';
            var p = location.pathname.split('/');
            document.cookie = cookieBase + '/';
            while (p.length > 0) {
                document.cookie = cookieBase + p.join('/');
                p.pop();
            };
            d.shift();
        }
    }

    window.location.reload();
}

function rowIsValid(cells, filter) {
    if (!filter) {
        return true;
    }
    var fl = filter.toLowerCase();
    for (var i = 0; i < cells.length; i++) {
        var e = cells[i];
        if (e.toLowerCase().includes(fl)) { return true; }
    }
    return false;
}

function renderRows(id) {
    var node_data = document.createElement("div");
    var kontoDaten = getKontoDaten();
    var data2 = kontoDaten.data[id];
    if (!data2) {
        return node_data;
    }
    var keys = Object.keys(data2.daten);
    // sort_by(keys, filter_by)
    for (var q = 0; q < keys.length; q++) {
        var e = keys[q];
        var row = kontoDaten.data[id].daten[e];
        if (!rowIsValid(row, filter_by)) { continue; }
        var row_node = document.createElement("div");
        row_node.dataset.index = e;

        if ((kontotyp == "admin" && id == "aenderungen") ||
           (kontotyp == "bearbeiter" && id == "aenderungen")) {

            var aenderung_id = row[0];
            var aenderung_name = row[1];
            var aenderung_email = row[2];
            var titel = row[6];
            var beschreibung = row[7];

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.flexDirection = "column";
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            check_node.dataset.id = aenderung_id;
            check_node.checked = selected.includes(aenderung_id);
            check_node.addEventListener('change', function(event) {
                if (event.currentTarget.checked) {
                    addToSelection(event.currentTarget);
                } else {
                    removeFromSelection(event.currentTarget);
                }
            });
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

            var cell_text1 = document.createElement("p");
            cell_text1.style.fontFamily = "monospace";

            var cell_text = document.createElement("a");
            var textnode1 = document.createTextNode(aenderung_id.substr(0, 6));
            cell_text.style.textDecoration = "underline";
            cell_text.href = "/aenderung/pdf/" + aenderung_id;
            cell_text.target = "_blank";
            cell_text.appendChild(textnode1);
            cell_text1.appendChild(cell_text);

            var textnode1 = document.createTextNode(" " + aenderung_name + " <");
            cell_text1.appendChild(textnode1);
            
            var cell_text = document.createElement("a");
            cell_text.style.textDecoration = "underline";
            cell_text.href = "mailto:" + aenderung_email;
            var textnode1 = document.createTextNode(aenderung_email);
            cell_text.appendChild(textnode1);
            cell_text1.appendChild(cell_text);

            var cell_text2 = document.createElement("p");
            var textnode2 = document.createTextNode(">");
            cell_text1.appendChild(textnode2);

            cell_node.appendChild(cell_text1);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "50%";
            cell_node.style.minWidth = "50%";
            cell_node.style.maxWidth = "50%";

            var cell_text = document.createElement("p");
            cell_text.style.fontFamily = "monospace";
            cell_node.classList.add("aenderung-titel");
            var textnode1 = document.createTextNode(titel.substr(0, 70));
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
                status = "Gewährt von " + gewaehrt_von;
                line2 = "am " + am;
            } else if (angefragt != "" && abgelehnt_von != "") {
                var am = new Date(Date.parse(am)).toLocaleDateString("de-DE", options);
                status = "Abgelehnt von " + abgelehnt_von;
                line2 = "am " + am;
            }

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.flexDirection = "column";
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            check_node.dataset.id = zugriff_id;
            check_node.checked = selected.includes(zugriff_id);
            check_node.addEventListener('change', function(event) {
                if (event.currentTarget.checked) {
                    addToSelection(event.currentTarget);
                } else {
                    removeFromSelection(event.currentTarget);
                }
            });
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
            var pubkey_fingerprint = row[4]; 

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.flexDirection = "column";
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            check_node.dataset.id = benutzer_email;
            check_node.checked = selected.includes(benutzer_email);
            check_node.addEventListener('change', function(event) {
                if (event.currentTarget.checked) {
                    addToSelection(event.currentTarget);
                } else {
                    removeFromSelection(event.currentTarget);
                }
            });
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

            var cell_text = document.createElement("select");
            cell_text.dataset.id = benutzer_email; 
            cell_text.onchange = function() { benutzerBearbeiten(this); };

            var select_values = [
                "admin",
                "bearbeiter",
                "gast",
            ];

            // Create and append the options
            for (var i = 0; i < select_values.length; i++) {
                var option = document.createElement("option");
                option.value = select_values[i];
                option.text = select_values[i];
                if (select_values[i] == benutzer_rechte) {
                    option.selected = true;
                }
                cell_text.appendChild(option);
            }
            cell_node.appendChild(cell_text);
            cell_node.style.alignSelf = "center";

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "25%";
            cell_node.style.minWidth = "25%";
            cell_node.style.maxWidth = "25%";

            var cell_text = document.createElement("button");
            cell_text.textContent = "Öffentlicher Schlüssel";
            cell_text.dataset.email = benutzer_email;
            cell_text.dataset.name = benutzer_name;

            if (pubkey_fingerprint == "") {
                cell_text.textContent = "Schlüsselpaar generieren";
                cell_text.onclick = function() { generiereSchluesselPaar(this); }
            } else {
                cell_text.textContent = "Schlüssel " + pubkey_fingerprint.substr(0, 6) + "";
            }
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            row_node.appendChild(non_check_node);

        } else if (kontotyp == "admin" && id == "bezirke") {
            
            var bezirk_id = row[0];
            var land = row[1];
            var amtsgericht = row[2];
            var bezirk = row[3];

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.flexDirection = "column";
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            check_node.dataset.id = bezirk_id;
            check_node.checked = selected.includes(bezirk_id);
            check_node.addEventListener('change', function(event) {
                if (event.currentTarget.checked) {
                    addToSelection(event.currentTarget);
                } else {
                    removeFromSelection(event.currentTarget);
                }
            });
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
        } else if (id == "meine-kontodaten") {

            var einstellung_id = e;
            var einstellung = row[1];
            var wert = row[2];

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.flexDirection = "column";
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";

            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            check_node.dataset.id = einstellung_id;
            check_node.checked = selected.includes(einstellung_id);
            check_node.addEventListener('change', function(event) {
                if (event.currentTarget.checked) {
                    addToSelection(event.currentTarget);
                } else {
                    removeFromSelection(event.currentTarget);
                }
            });
            check_uncheck_all_node_div.appendChild(check_node);
            row_node.appendChild(check_uncheck_all_node_div);

            var non_check_node = document.createElement("div");

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "50%";
            cell_node.style.minWidth = "50%";
            cell_node.style.maxWidth = "50%";

            var cell_text = document.createElement("p");
            var textnode1 = document.createTextNode(einstellung);
            cell_text.appendChild(textnode1);
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            var cell_node = document.createElement("div");
            cell_node.classList.add("row-cell");
            cell_node.style.width = "50%";
            cell_node.style.minWidth = "50%";
            cell_node.style.maxWidth = "50%";

            var cell_text = document.createElement("input");
            cell_text.value = wert;
            cell_text.dataset.id = einstellung_id;
            cell_text.onchange = function() { editConfigValue(this); }
            cell_node.appendChild(cell_text);

            non_check_node.appendChild(cell_node);

            row_node.appendChild(non_check_node);
        } else if (id == "blaetter") {

            var land = row[0];
            var amtsgericht = row[1];
            var bezirk = row[2];
            var blatt = row[3];
            var blatt_id = land + "/" + amtsgericht + "/" + bezirk + "/" + blatt;

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.flexDirection = "column";
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            check_node.dataset.id = blatt_id;
            check_node.checked = selected.includes(blatt_id);
            check_node.addEventListener('change', function(event) {
                if (event.currentTarget.checked) {
                    addToSelection(event.currentTarget);
                } else {
                    removeFromSelection(event.currentTarget);
                }
            });
            check_uncheck_all_node_div.appendChild(check_node);
            row_node.appendChild(check_uncheck_all_node_div);
            
            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            row_node.appendChild(check_uncheck_all_node_div);

            var non_check_node = document.createElement("div");

            var values = [land, amtsgericht, bezirk, blatt];

            for (var q = 0; q < values.length; q++) {

                var e = values[q];
                var cell_node = document.createElement("div");
                cell_node.classList.add("row-cell");
                cell_node.style.width = "25%";
                cell_node.style.minWidth = "25%";
                cell_node.style.maxWidth = "25%";
                cell_node.style.flexDirection = "row";

                var cell_text = document.createElement("p");
                cell_text.appendChild(document.createTextNode(e));
                cell_node.appendChild(cell_text);

                if (q == 3) {
                    var spacer = document.createElement("div");
                    spacer.style.display = "flex";
                    spacer.style.flexGrow = "1";
                    cell_node.appendChild(spacer);
    
                    var pdf_link = document.createElement("a");
                    pdf_link.href = "/download/pdf/" + amtsgericht + "/" + bezirk + "/" + blatt;
                    pdf_link.style.textDecoration = "underline";
                    pdf_link.style.fontSize = "0.8rem";
                    pdf_link.style.marginRight = "5px";
                    pdf_link.appendChild(document.createTextNode("PDF"));
                    cell_node.appendChild(pdf_link);
    
                    var gbx_link = document.createElement("a");
                    gbx_link.href = "/download/gbx/" + amtsgericht + "/" + bezirk + "/" + blatt;
                    gbx_link.style.textDecoration = "underline";
                    gbx_link.style.fontSize = "0.8rem";
                    gbx_link.appendChild(document.createTextNode("GBX"));
                    cell_node.appendChild(gbx_link);    
                }

                non_check_node.appendChild(cell_node);
            }

            row_node.appendChild(non_check_node);
        } else if (id == "abonnements") {
            var typ = row[0];
            var text = row[1];
            var amtsgericht = row[2];
            var bezirk = row[3];
            var blatt = row[4];
            var aktenzeichen = row[5];

            var check_uncheck_all_node_div = document.createElement("div");
            check_uncheck_all_node_div.style.flexDirection = "column";
            check_uncheck_all_node_div.style.padding = "5px 10px";
            check_uncheck_all_node_div.style.flexGrow = "0";
            check_uncheck_all_node_div.style.maxWidth = "18px";
            check_uncheck_all_node_div.style.minWidth = "18px";
            var check_node = document.createElement("input");
            check_node.type = "checkbox";
            check_node.style.minWidth = "15px";
            check_node.dataset.id = e;
            check_node.checked = selected.includes(e);
            check_node.addEventListener('change', function(event) {
                if (event.currentTarget.checked) {
                    addToSelection(event.currentTarget);
                } else {
                    removeFromSelection(event.currentTarget);
                }
            });
            check_uncheck_all_node_div.appendChild(check_node);
            row_node.appendChild(check_uncheck_all_node_div);

            var non_check_node = document.createElement("div");

            var values = [typ, text, amtsgericht, bezirk, blatt, aktenzeichen];

            for (var q = 0; q < values.length; q++) {
                var e = values[q];
                var cell_node = document.createElement("div");
                cell_node.classList.add("row-cell");
                var pct = 100 / values.length;
                cell_node.style.width = pct + "%";
                cell_node.style.minWidth = pct + "%";
                cell_node.style.maxWidth = pct + "%";
                var cell_text = document.createElement("p");
                var textnode1 = document.createTextNode(e);
                cell_text.appendChild(textnode1);
                cell_node.appendChild(cell_text);
                non_check_node.appendChild(cell_node);
            }

            row_node.appendChild(non_check_node);
        }

        node_data.appendChild(row_node);
    }

    if (kontotyp == "bearbeiter" && id == "meine-kontodaten") {

        var pubkey_fingerprint = kontoDaten.data["kontodaten-extra"].daten["konto.publickey"][0];
        var benutzer_email = kontoDaten.data["kontodaten-extra"].daten["konto.email"][0];
        var benutzer_name = kontoDaten.data["kontodaten-extra"].daten["konto.name"][0];

        var cell_text = document.createElement("button");
        cell_text.textContent = "Öffentlicher Schlüssel";
        cell_text.dataset.email = benutzer_email;
        cell_text.dataset.name = benutzer_name;

        if (pubkey_fingerprint == "") {
            cell_text.textContent = "Schlüsselpaar generieren";
            cell_text.onclick = function() { generiereSchluesselPaar(this); }
        } else {
            cell_text.textContent = "Schlüssel " + pubkey_fingerprint.substr(0, 6) + "";
        }
        node_data.appendChild(cell_text);
    }
    return node_data;
}  

function editConfigValue(target) {
    var target_id = target.dataset.id;
    if (!target) {
        return;
    }
    postToServer("konfiguration-bearbeiten", [target_id, target.value]);
}

function saveFile(fileName,urlFile){
    let a = document.createElement("a");
    a.style = "display: none";
    document.body.appendChild(a);
    a.href = urlFile;
    a.download = fileName;
    a.click();
    window.URL.revokeObjectURL(urlFile);
    a.remove();
}

function generiereSchluesselPaar(target) {
    var email = target.dataset.email;
    var name = target.dataset.name;
    var auth = document.getElementById("token-id").dataset.tokenId;
    if (!auth) {
        return;
    }

    var http = new XMLHttpRequest();
    http.open('POST', '/konto-generiere-schluessel', true);
    http.setRequestHeader('Content-type', 'application/json');
    http.onreadystatechange = function() {
        if (http.readyState == 4 && http.status == 200) {
            var object = JSON.parse(http.responseText);
            if (object.status == "ok") {
                var privatekey = object.private.join('\r\n');
                var blobData = new Blob([privatekey], {type: "text/plain"});
                var url = window.URL.createObjectURL(blobData);
                saveFile("privat-" + name + "-" + object.fingerprint.substr(0, 6) + ".txt", url);
                
                try {
                    var publickey = object.public.join('\r\n');
                    var blobData2 = new Blob([publickey], {type: "text/plain"});
                    var url2 = window.URL.createObjectURL(blobData2);
                    saveFile("public-" + name + "-" + object.fingerprint.substr(0, 6) + ".txt", url2);      
                } catch (error) { }

                postToServer("benutzer-bearbeite-pubkey", [email, object.public.join('\r\n')]);

            } else if (object.status == "error") {
                console.error("" + object.code + ": " + object.text);
            }
        }
    }
    http.send(JSON.stringify({
        auth: auth,
        name: name,
        email: email,
    }));
}

function renderActions(id) {
    var actions_data = document.createElement("div");
    if (kontotyp == "admin" && id == "aenderungen") {

    } else if (kontotyp == "admin" && id == "zugriffe") {
        var genehmigen = document.createElement("button");
        genehmigen.textContent = "Zugriff genehmigen";
        genehmigen.onclick = function() { zugriffGenehmigen(); }
        actions_data.appendChild(genehmigen);

        var ablehnen = document.createElement("button");
        ablehnen.textContent = "Zugriff ablehnen";
        ablehnen.onclick = function() { zugriffAblehnen(); }
        actions_data.appendChild(ablehnen);
    } else if (kontotyp == "admin" && id == "benutzer") {

        var change = document.createElement("button");
        change.textContent = "Neuen Benutzer anlegen";
        change.onclick = function(){ benutzerNeu(); };
        actions_data.appendChild(change);

        var loeschen = document.createElement("button");
        loeschen.textContent = "Benutzer löschen";
        loeschen.onclick = function() { benutzerLoeschen(); }
        actions_data.appendChild(loeschen);
    } else if (kontotyp == "admin" && id == "bezirke") {
        var bezirk_new = document.createElement("label");
        bezirk_new.htmlFor = "bezirke-von-csv-laden";
        bezirk_new.textContent = "Bezirke von CSV laden";
        bezirk_new.classList.add("custom-file-upload");
        actions_data.appendChild(bezirk_new);

        var bezirk_new = document.createElement("input");
        bezirk_new.type = "file";
        bezirk_new.id = "bezirke-von-csv-laden";
        bezirk_new.onchange = function() { bezirkNeuVonCsv(this); }
        actions_data.appendChild(bezirk_new);

        var loeschen = document.createElement("button");
        loeschen.textContent = "Bezirk löschen";
        loeschen.onclick = function() { bezirkLoeschen(); }
        actions_data.appendChild(loeschen);
    } else if (id == "blaetter") {
        var herunterladen = document.createElement("button");
        herunterladen.textContent = "Ausgewählte Blätter als .zip herunterladen";
        herunterladen.onclick = function(){ blaetterAlsZip(); };
        actions_data.appendChild(herunterladen);
    } else if (id == "abonnements") {
        var neu = document.createElement("button");
        neu.textContent = "Neues Abonnement";
        neu.onclick = function(){ aboNeu(); };
        actions_data.appendChild(neu);

        var loeschen = document.createElement("button");
        loeschen.textContent = "Ausgewählte Abonnements beenden";
        loeschen.onclick = function(){ aboBeenden(); };
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

function postToServer(aktion, daten) {
    var auth = document.getElementById("token-id").dataset.tokenId;
    if (!auth) {
        return;
    }
    var http = new XMLHttpRequest();
    http.open('POST', '/konto', true);
    http.setRequestHeader('Content-type', 'application/json');
    http.onreadystatechange = function() {
        if (http.readyState == 4 && http.status == 200) {
            var object = JSON.parse(http.responseText);
            if (object.status == "ok") {
                setKontoDaten(object);
                renderMainTable();
            } else if (object.status == "error") {
                console.error("" + object.code + ": " + object.text);
            }
        }
    }
    http.send(JSON.stringify({
        auth: auth,
        aktion: aktion,
        daten: daten,
    }));
}

function zugriffZurueckziehen() {
    postToServer("zugriff-zurueckziehen", selected);
}

function zugriffGenehmigen() {
    postToServer("zugriff-genehmigen", selected);
}

function zugriffAblehnen() {
    postToServer("zugriff-ablehnen", selected);
}

function blaetterAlsZip() {
    var auth = document.getElementById("token-id").dataset.tokenId;
    if (!auth) {
        return;
    }
    var http = new XMLHttpRequest();
    http.open('POST', '/konto', true);
    http.setRequestHeader('Content-type', 'application/json');
    http.responseType = "arraybuffer";

    http.onreadystatechange = function() {
        if (http.readyState == 4 && http.status == 200) {
            const byteArray = new Uint8Array(http.response);
            downloadBlob(byteArray, "download.zip", 'application/zip');
        }
    }
    http.send(JSON.stringify({
        auth: auth,
        aktion: "blaetter-als-zip",
        daten: selected,
    }));
}

function benutzerBearbeiten(target) {
    var value = target.value;
    var s = target.options[target.selectedIndex].text;
    if (!s) {
        return;
    }
    var target_ids = [s, target.dataset.id];
    for (var i = 0; i < selected.length; i++) {
        const s = selected[i];
        target_ids.push(s);
    }
    postToServer("benutzer-bearbeite-kontotyp", target_ids);
}

function aboNeu() {
    var typ = window.prompt("Abo-Typ (email | webhook)", "");
    if (!typ) { return; }
    if (typ != "email" && typ != "webhook") {
        alert("Falscher Typ, bitte 'email' oder 'webhook' eingeben");
        return;
    }

    var text = null;
    if (kontotyp == "admin" && typ == "email") {
        text = window.prompt("E-Mail", "");
    } else if (typ == "webhook") {
        text = window.prompt("Webhook-URL", "");
    } else {
        text = "";
    }
    if (!text) { return; }
    
    var amtsgericht = window.prompt("Amtsgericht", "");
    if (!amtsgericht) { return; }
    var bezirk = window.prompt("Bezirk", "");
    if (!bezirk) { return; }
    var blatt = window.prompt("Blatt", "");
    if (!blatt) { return; }
    var aktenzeichen = window.prompt("Aktenzeichen", "");
    if (!aktenzeichen) { return; }
    postToServer("abo-neu", [typ, text, amtsgericht, bezirk, blatt, aktenzeichen]);
}

function aboLoeschen() {
    postToServer("abo-loeschen", selected);
}

function benutzerNeu() {
    var name = window.prompt("Name", "");
    if (!name) { return; }
    var email = window.prompt("E-Mail", "");
    if (!email) { return; }
    var passwort = window.prompt("Passwort", "");
    if (!passwort) { return; }
    postToServer("benutzer-neu", [name, email, passwort]);
}

function benutzerLoeschen() {
    postToServer("benutzer-loeschen", selected);
}

function getCsvValuesFromLine(line) {
    var values = line[0].split(',');
    value = values.map(function(value){
        return value.replace(/\"/g, '');
    });
    return values;
}

function bezirkNeuVonCsv(event) {

    var reader = new FileReader()
    reader.onload = () => {
        var lines = reader.result.split('\n');
        var values = [];
        for (var i = 0; i < lines.length; i++) {
            var line = lines[i];
            var line_elements = line.split(',');
            for (let j = 0; j < line_elements.length; j++) {
                const v = line_elements[j];
                values.push(v);
            }
        }
        postToServer("bezirk-neu", values);
    }
    reader.readAsBinaryString(event.files[0]);
}

function bezirkLoeschen() {
    postToServer("bezirk-loeschen", selected);
}

renderSidebar();
renderMainTable();
document.getElementById("main-table-filter").onchange = function() { updateFilter(this); }
document.getElementById("main-table-filter").oninput = function() { updateFilter(this); }