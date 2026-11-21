'use strict';

(function () {
  var orderForm = document.getElementById('order-form');
  if (!orderForm) return;

  var lines = orderForm.querySelector('.order-lines');
  var addLineBtn = document.getElementById('add-line');

  function updateRemoveButtons() {
    var rows = lines.querySelectorAll('.order-line');
    rows.forEach(function (row) {
      var btn = row.querySelector('.remove-line');
      if (btn) {
        btn.disabled = rows.length <= 1;
      }
    });
  }

  function addLine() {
    var first = lines.querySelector('.order-line');
    if (!first) return;
    var clone = first.cloneNode(true);
    var select = clone.querySelector('select');
    var qty = clone.querySelector('input');
    if (select) select.selectedIndex = 0;
    if (qty) qty.value = '1';
    lines.appendChild(clone);
    updateRemoveButtons();
  }

  addLineBtn.addEventListener('click', addLine);

  lines.addEventListener('click', function (e) {
    if (e.target && e.target.classList.contains('remove-line')) {
      var row = e.target.closest('.order-line');
      if (lines.querySelectorAll('.order-line').length > 1) {
        row.parentNode.removeChild(row);
        updateRemoveButtons();
      }
    }
  });

  updateRemoveButtons();
})();
