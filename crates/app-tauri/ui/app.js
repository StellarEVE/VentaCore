// VentaCore Studio Webview Front-End

document.addEventListener('DOMContentLoaded', () => {
  // 1. Navigation switching
  const tabs = document.querySelectorAll('.nav-tab');
  const pages = document.querySelectorAll('.page-view');

  function switchPage(pageId) {
    tabs.forEach(t => {
      if (t.dataset.page === pageId) {
        t.classList.add('active');
      } else {
        t.classList.remove('active');
      }
    });

    pages.forEach(p => {
      if (p.id === `page-${pageId}`) {
        p.classList.add('active');
      } else {
        p.classList.remove('active');
      }
    });
  }

  tabs.forEach(tab => {
    tab.addEventListener('click', () => switchPage(tab.dataset.page));
  });

  // Keyboard shortcuts (Ctrl+1 through Ctrl+4)
  window.addEventListener('keydown', (e) => {
    if (e.ctrlKey) {
      if (e.key === '1') { e.preventDefault(); switchPage('media'); }
      if (e.key === '2') { e.preventDefault(); switchPage('cut'); }
      if (e.key === '3') { e.preventDefault(); switchPage('edit'); }
      if (e.key === '4') { e.preventDefault(); switchPage('export'); }
    }

    // Spacebar play/pause
    if (e.code === 'Space' && e.target.tagName !== 'INPUT') {
      e.preventDefault();
      togglePlayback();
    }
  });

  // 2. Playback state
  let isPlaying = false;
  const playBtn = document.getElementById('btn-play-pause');
  const scrubBar = document.getElementById('source-scrub');
  const timecodeDisplay = document.getElementById('source-timecode');

  function togglePlayback() {
    isPlaying = !isPlaying;
    playBtn.textContent = isPlaying ? '⏸' : '▶';
    if (window.__TAURI__) {
      window.__TAURI__.core.invoke('toggle_playback').catch(() => {});
    }
  }

  playBtn.addEventListener('click', togglePlayback);

  // 3. Mock Footage Pool & Demo interaction
  const footageList = document.getElementById('footage-list');
  const sourceTitle = document.getElementById('source-title');
  const sourceRes = document.getElementById('source-res');
  const placeholder = document.getElementById('source-placeholder');
  const videoElem = document.getElementById('source-video-element');

  const demoClips = [
    { id: 1, name: "Concert_Drone_Intro.mp4", duration: "0:24", res: "1920x1080", path: "C:/footage/drone.mp4" },
    { id: 2, name: "Crowd_Cheering_Night.mp4", duration: "1:15", res: "1920x1080", path: "C:/footage/crowd.mp4" },
    { id: 3, name: "Lead_Guitar_Solo_Close.mp4", duration: "0:42", res: "3840x2160", path: "C:/footage/solo.mp4" }
  ];

  function renderFootagePool(clips) {
    footageList.innerHTML = '';
    clips.forEach(clip => {
      const item = document.createElement('div');
      item.className = 'footage-item';
      item.innerHTML = `
        <div class="footage-meta">
          <span class="footage-name">🎬 ${clip.name}</span>
          <span class="footage-info">${clip.duration} • ${clip.res}</span>
        </div>
        <button class="btn btn-sm btn-secondary">Select</button>
      `;

      item.addEventListener('click', () => {
        document.querySelectorAll('.footage-item').forEach(i => i.classList.remove('selected'));
        item.classList.add('selected');
        sourceTitle.textContent = clip.name;
        sourceRes.textContent = clip.res;
        placeholder.innerHTML = `
          <span class="placeholder-icon">🎬</span>
          <span>Playing: <strong>${clip.name}</strong></span>
          <span style="font-size: 11px; color: var(--accent);">${clip.res} (Native Hardware Decode)</span>
        `;
      });

      footageList.appendChild(item);
    });
  }

  renderFootagePool(demoClips);

  // 4. File picker hooks
  document.getElementById('btn-browse-music').addEventListener('click', () => {
    const songCard = document.getElementById('song-card');
    const songName = document.getElementById('song-name');
    const dropZone = document.getElementById('drop-zone-music');
    songCard.classList.remove('hidden');
    dropZone.classList.add('hidden');
    songName.textContent = "Neon_Starlight_Master.mp3";
    document.getElementById('song-duration').textContent = "3:42";
    document.getElementById('bpm-display').textContent = "126.0 BPM";
  });

  document.getElementById('btn-add-videos').addEventListener('click', () => {
    alert("In Tauri, clicking 'Add Videos' opens the native Windows file chooser dialog via Tauri plugins or rfd.");
  });

  // Time ticker
  let tick = 0;
  setInterval(() => {
    if (isPlaying) {
      tick++;
      const secs = (tick / 30) % 60;
      const mins = Math.floor(tick / (30 * 60));
      const frames = tick % 30;
      timecodeDisplay.textContent = `00:${String(mins).padStart(2, '0')}:${String(Math.floor(secs)).padStart(2, '0')}:${String(frames).padStart(2, '0')}`;
      scrubBar.value = (tick % 300) / 3;
    }
  }, 33);
});
