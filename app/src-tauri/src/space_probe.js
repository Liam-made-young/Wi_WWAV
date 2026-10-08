// The proof's questions to a page (space_pages.rs, `proof`): what it holds,
// whether it remembers the last launch, and whether pressing play plays.
(function () {
  var space = window.__wwavSpace;
  var asked = window.__wwavProof;
  if (!space || !asked) return;
  var began = Date.now();
  // Every answer names its case, so one that arrives late is not taken for the next page's.
  function say(proof) {
    proof.name = asked.name;
    proof.after = Date.now() - began;
    space.say({ proof: proof });
  }
  // The proof presses for you.
  space.dock(true);

  function deep(root, selector, found) {
    found = found || [];
    try {
      root.querySelectorAll(selector).forEach(function (el) { found.push(el); });
      root.querySelectorAll('*').forEach(function (el) {
        if (el.shadowRoot) deep(el.shadowRoot, selector, found);
      });
    } catch (e) {}
    return found;
  }

  function playing() {
    var list = [];
    function one(el, how) {
      list.push({
        how: how,
        tag: el.tagName,
        paused: el.paused,
        time: Math.round(el.currentTime * 100) / 100,
        length: isFinite(el.duration) ? Math.round(el.duration) : null,
        error: el.error ? el.error.code : null,
      });
    }
    space.media.forEach(function (el) { one(el, 'played'); });
    deep(document, 'audio, video').forEach(function (el) {
      if (!space.media.has(el)) one(el, 'in the page');
    });
    return list;
  }

  function remembered() {
    var local = null;
    try { local = window.localStorage.getItem('wwav_probe'); } catch (e) { local = 'refused'; }
    return { cookie: /(^|; )wwav_probe=/.test(document.cookie), local: local };
  }

  function facts() {
    var body = document.body;
    return {
      title: document.title,
      url: window.location.href,
      links: document.links.length,
      letters: body ? (body.innerText || '').length : 0,
      frames: document.querySelectorAll('iframe').length,
      playing: playing(),
      // A player that shows nothing to press: what it does hold.
      buttons: asked.press ? deep(document, 'button, [role=button], a').slice(0, 14).map(label) : undefined,
      holds: asked.press && body ? body.innerHTML.length : undefined,
      parts: asked.press && body ? Array.prototype.slice.call(body.querySelectorAll('*'), 0, 400).map(function (el) { return el.tagName.toLowerCase(); }).filter(function (t, i, all) { return t.indexOf('-') > 0 && all.indexOf(t) === i; }).slice(0, 12) : undefined,
    };
  }

  var before = remembered();
  var pressed = null;
  var youtube = null;

  function press() {
    var yt = window.__wwavYouTube;
    if (yt && yt.playVideo) {
      pressed = 'the YouTube player';
      try { yt.setVolume(2); yt.playVideo(); } catch (e) { pressed = 'the YouTube player refused: ' + e; }
      return;
    }
    // The player's own button, not a link that says "Play on …".
    var buttons = deep(document, 'button').filter(function (b) {
      var words = [b.getAttribute('aria-label'), b.getAttribute('data-testid'), b.title, String(b.className || ''), (b.textContent || '').trim().slice(0, 12)]
        .join(' ').toLowerCase().trim();
      return /play-pause|^play\b|\bplay$|play-button|\bplay\b/.test(words) && !/playlist|display| on /.test(words);
    });
    button = buttons[0] || null;
    if (button) {
      pressed = label(button);
      button.click();
      space.media.forEach(function (el) { try { el.volume = 0.02; } catch (e) {} });
    }
  }

  var button = null;
  function label(b) {
    var text = (b.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 24);
    return [b.getAttribute('aria-label'), b.getAttribute('data-testid'), b.title, String(b.className || '').slice(0, 40), text].join(' | ').slice(0, 120);
  }

  say({ at: 'start', remembered: before, facts: facts() });
  if (asked.press) setTimeout(press, 2500);
  setTimeout(function () {
    space.media.forEach(function (el) { try { el.volume = 0.02; } catch (e) {} });
  }, 3200);
  setTimeout(function () {
    var yt = window.__wwavYouTube;
    if (yt && yt.getPlayerState) {
      try { youtube = { state: yt.getPlayerState(), time: Math.round(yt.getCurrentTime() * 100) / 100, muted: yt.isMuted() }; } catch (e) {}
      try { yt.pauseVideo(); } catch (e) {}
    }
    var end = { at: 'end', pressed: pressed, button: button ? label(button) : null, youtube: youtube, facts: facts() };
    space.media.forEach(function (el) { try { el.pause(); } catch (e) {} });
    try {
      if (!before.cookie) document.cookie = 'wwav_probe=' + Date.now() + '; max-age=31536000; path=/; SameSite=Lax';
      if (!before.local) window.localStorage.setItem('wwav_probe', String(Date.now()));
    } catch (e) {}
    say(end);
  }, 10500);
})();
