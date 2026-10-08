// Put in every page Space opens, before the page's own scripts
// (space_pages.rs). It gives the page one way to say something to the app,
// and makes the page part of the sky until you are in it.
(function () {
  if (window.__wwavSpace) return;

  // Saying: ask to go to an address that is never a place. The app reads it
  // and refuses, so the page stays where it is. Lines said close together
  // travel as one request, since a second request would replace the first.
  var waiting = [];
  var timer = null;
  function send() {
    timer = null;
    if (!waiting.length) return;
    var lines = waiting;
    waiting = [];
    try {
      window.location.href = 'wwavspace://say/?' + encodeURIComponent(JSON.stringify(lines));
    } catch (e) {}
  }
  function say(line) {
    waiting.push(line);
    if (!timer) timer = setTimeout(send, 16);
  }

  // Every sound or picture the page starts, so the app can tell what plays.
  var media = new Set();
  try {
    var play = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function () {
      media.add(this);
      return play.apply(this, arguments);
    };
  } catch (e) {}

  var docked = false;
  window.__wwavSpace = {
    say: say,
    media: media,
    dock: function (on) {
      docked = !!on;
      window.dispatchEvent(new CustomEvent('wwavspace-dock', { detail: docked }));
    },
    docked: function () {
      return docked;
    },
  };

  // Out in the sky, scrolling over the page moves you toward it or away.
  window.addEventListener(
    'wheel',
    function (e) {
      if (docked) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      say({ wheel: [e.deltaX, e.deltaY] });
    },
    { capture: true, passive: false },
  );

  // And the page can't be pressed by accident: a click goes in.
  ['mousedown', 'mouseup', 'click', 'dblclick', 'contextmenu', 'pointerdown', 'pointerup'].forEach(function (type) {
    window.addEventListener(
      type,
      function (e) {
        if (docked) return;
        e.preventDefault();
        e.stopImmediatePropagation();
        if (type === 'click') say({ enter: 1 });
      },
      true,
    );
  });

  // A pinch: toward the page in the sky; outward from inside it, leave.
  var last = 1;
  window.addEventListener(
    'gesturestart',
    function (e) {
      e.preventDefault();
      last = 1;
    },
    true,
  );
  window.addEventListener(
    'gesturechange',
    function (e) {
      e.preventDefault();
      if (!docked && last > 0) say({ pinch: e.scale / last });
      last = e.scale;
    },
    true,
  );
  window.addEventListener(
    'gestureend',
    function (e) {
      e.preventDefault();
      if (docked && e.scale < 0.75) say({ leave: 1 });
    },
    true,
  );

  // Esc leaves, unless the page used it (closing its own dialog, or a film
  // filling the screen).
  window.addEventListener(
    'keydown',
    function (e) {
      if (docked && e.key === 'Escape' && !e.defaultPrevented && !document.fullscreenElement) say({ leave: 1 });
    },
    false,
  );

  function head() {
    say({ title: document.title, url: window.location.href });
  }
  document.addEventListener('DOMContentLoaded', head);
  window.addEventListener('load', head);
})();
