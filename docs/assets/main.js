// ytbuch — пишещ се терминал в hero и бутон „копирай“ за deploy командите.
// Без JS цялото съдържание си остава видимо; анимацията се пуска само
// ако потребителят не е поискал reduced motion.

(function () {
  "use strict";

  var reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Пишещ се терминал: командите се изписват буква по буква, изходът
  // след всяка команда се появява наведнъж. Курсорът остава да мига накрая.
  function typeHero() {
    var term = document.getElementById("hero-term");
    if (!term || reduceMotion) return;
    var lines = Array.prototype.slice.call(term.querySelectorAll(".t-line"));
    var texts = lines.map(function (el) {
      var t = el.textContent;
      if (!el.querySelector(".cursor")) {
        // Редът с мигащия курсор не се пипа — span-ът трябва да остане.
        el.textContent = "";
      }
      el.style.visibility = "hidden";
      return t;
    });
    var i = 0;

    function nextLine() {
      if (i >= lines.length) return;
      var el = lines[i];
      var text = texts[i];
      var isCmd = el.classList.contains("cmd");
      el.style.visibility = "visible";
      i += 1;
      if (el.querySelector(".cursor")) {
        setTimeout(nextLine, 160);
        return;
      }
      if (!isCmd) {
        // Изходът се появява наведнъж след командата.
        el.textContent = text;
        setTimeout(nextLine, 160);
        return;
      }
      var pos = 0;
      (function typeChar() {
        pos += 1;
        el.textContent = text.slice(0, pos);
        if (pos < text.length) {
          setTimeout(typeChar, 45);
        } else {
          setTimeout(nextLine, 260);
        }
      })();
    }

    setTimeout(nextLine, 500);
  }

  // Бутонът „копирай“ е скрит без JS; с JS копира командите в клипборда.
  function initCopy() {
    var buttons = document.querySelectorAll(".copy[data-copy]");
    Array.prototype.forEach.call(buttons, function (btn) {
      btn.hidden = false;
      btn.addEventListener("click", function () {
        var text = btn.getAttribute("data-copy");
        function done() {
          var old = btn.textContent;
          btn.textContent = "[копирано]";
          setTimeout(function () {
            btn.textContent = old;
          }, 1600);
        }
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(text).then(done, function () {});
        }
      });
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      typeHero();
      initCopy();
    });
  } else {
    typeHero();
    initCopy();
  }
})();
