/* S2C MAROC — progressive enhancement only; every form works without JS. */
(function () {
  "use strict";

  var money = function (n) {
    return (Math.round(n * 100) / 100).toFixed(2);
  };

  function mutate(fn) { fn(); }

  /* ---------------- document line editor ---------------- */
  function recalc(wrap) {
    var sumSub = document.getElementById("sum-sub");
    var sumTax = document.getElementById("sum-tax");
    var sumTotal = document.getElementById("sum-total");
    var sub = 0, tax = 0;

    wrap.querySelectorAll("tr.line-row").forEach(function (row) {
      var q = parseFloat((row.querySelector('input[name="item_qty"]') || {}).value) || 0;
      var p = parseFloat((row.querySelector('input[name="item_price"]') || {}).value) || 0;
      var t = parseFloat((row.querySelector('input[name="item_tax"]') || {}).value) || 0;
      var net = q * p;
      var lineTax = (net * t) / 100;
      sub += net;
      tax += lineTax;
      var cell = row.querySelector(".line-total");
      if (cell) cell.textContent = money(net);
    });

    if (sumSub) sumSub.textContent = money(sub);
    if (sumTax) sumTax.textContent = money(tax);
    if (sumTotal) sumTotal.textContent = money(sub + tax);
  }

  function initLines() {
    var wrap = document.querySelector(".lines-wrap");
    if (!wrap) return;
    var body = wrap.querySelector("#lines-body");
    var tpl = wrap.querySelector("#line-tpl");
    var addBtn = wrap.querySelector("#add-line");
    var mode = wrap.getAttribute("data-mode") || "sale";

    if (addBtn && tpl && body) {
      addBtn.addEventListener("click", function () {
        body.appendChild(tpl.content.cloneNode(true));
        recalc(wrap);
      });
    }

    wrap.addEventListener("click", function (e) {
      if (e.target.classList.contains("remove-line")) {
        var row = e.target.closest("tr.line-row");
        if (row && body && body.querySelectorAll("tr.line-row").length > 1) {
          row.remove();
          recalc(wrap);
        }
      }
    });

    wrap.addEventListener("change", function (e) {
      var row = e.target.closest("tr.line-row");
      if (!row) return;
      if (e.target.classList.contains("line-product")) {
        var opt = e.target.options[e.target.selectedIndex];
        if (!opt) return;
        var price = opt.getAttribute(mode === "cost" ? "data-cost" : "data-price");
        var priceInput = row.querySelector('input[name="item_price"]');
        var descInput = row.querySelector('input[name="item_desc"]');
        if (price && priceInput && (parseFloat(priceInput.value) || 0) === 0) {
          priceInput.value = price;
        }
        if (descInput && !descInput.value && opt.textContent) {
          descInput.value = opt.textContent.replace(/\s*\(.*\)\s*$/, "").trim();
        }
      }
      recalc(wrap);
    });

    wrap.addEventListener("input", function (e) {
      if (e.target.name === "item_qty" || e.target.name === "item_price" || e.target.name === "item_tax") {
        recalc(wrap);
      }
    });

    recalc(wrap);
  }

  /* ---------------- flash messages ---------------- */
  function initFlash() {
    document.querySelectorAll(".flash").forEach(function (el) {
      setTimeout(function () {
        el.style.transition = "opacity .4s";
        el.style.opacity = "0";
        setTimeout(function () { el.remove(); }, 400);
      }, 6000);
    });
  }

  /* ---------------- double submit guard ---------------- */
  function initForms() {
    document.querySelectorAll("form").forEach(function (form) {
      form.addEventListener("submit", function () {
        var btn = form.querySelector('button[type="submit"]');
        if (btn) {
          setTimeout(function () { btn.disabled = true; }, 0);
        }
      });
    });
  }

  /* ---------------- delete confirmation ---------------- */
  function initConfirm() {
    document.querySelectorAll("form").forEach(function (form) {
      if (form.action && /\/delete$/.test(form.action)) {
        form.addEventListener("submit", function (e) {
          if (!window.confirm("Delete this record?")) e.preventDefault();
        });
      }
    });
  }

  /* ---------------- Bluetooth ESC/POS printing ---------------- */
  /* Mini printers expose the common ESC/POS service 0x18F0. */
  function initBluetooth() {
    var btn = document.querySelector(".bt-print");
    if (!btn) return;
    if (!navigator.bluetooth) {
      btn.addEventListener("click", function () {
        alert("Web Bluetooth is not available in this browser. Use the ESC/POS link with a USB/serial printer.");
      });
      return;
    }
    btn.addEventListener("click", async function () {
      var url = btn.getAttribute("data-url");
      try {
        btn.disabled = true;
        var device = await navigator.bluetooth.requestDevice({
          filters: [{ services: [0x18f0] }],
          optionalServices: [0x18f0, 0xff00, 0x1101]
        });
        var server = await device.gatt.connect();
        var service = await server.getPrimaryService(0x18f0);
        var characteristic = await service.getCharacteristic(0x2af1);
        var resp = await fetch(url, { credentials: "same-origin" });
        if (!resp.ok) throw new Error("could not fetch ticket data");
        var buffer = new Uint8Array(await resp.arrayBuffer());
        var chunk = 100;
        for (var i = 0; i < buffer.length; i += chunk) {
          await characteristic.writeValue(buffer.slice(i, i + chunk));
        }
        btn.textContent = "Printed";
      } catch (err) {
        console.error(err);
        alert("Bluetooth printing failed: " + err.message);
      } finally {
        setTimeout(function () { btn.disabled = false; }, 1500);
      }
    });
  }

  mutate(function () {
    document.addEventListener("DOMContentLoaded", function () {
      initLines();
      initFlash();
      initForms();
      initConfirm();
      initBluetooth();
    });
  });
})();
