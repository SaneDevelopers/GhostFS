<script>
  import { onMount } from 'svelte';
  
  // Mock Tauri functions for web-only demo
  const invoke = async (cmd, args) => {
    console.log(`Mock invoke: ${cmd}`, args);
    
    if (cmd === 'detect_filesystem') {
      return 'xfs';
    }
    if (cmd === 'start_scan') {
      return {
        id: 'demo-session-123',
        fs_type: 'XFS',
        device_path: args.imagePath,
        created_at: new Date().toISOString(),
        files_found: 42,
        recoverable_files: 38,
        confidence_threshold: args.confidence
      };
    }
    if (cmd === 'get_session_files') {
      return [
        { id: 1, name: 'document.pdf', size: 2048576, confidence: 0.95, type: 'file', selected: false, is_recoverable: true },
        { id: 2, name: 'photo.jpg', size: 1024000, confidence: 0.87, type: 'file', selected: false, is_recoverable: true },
        { id: 3, name: 'backup.tar', size: 5242880, confidence: 0.72, type: 'file', selected: false, is_recoverable: true },
        { id: 4, name: 'config.xml', size: 4096, confidence: 0.65, type: 'file', selected: false, is_recoverable: true },
        { id: 5, name: 'database.db', size: 10485760, confidence: 0.43, type: 'file', selected: false, is_recoverable: false }
      ];
    }
    if (cmd === 'recover_session_files') {
      return {
        success: true,
        message: 'Recovery completed successfully',
        files_recovered: args.fileIds ? args.fileIds.length : 5,
        total_size: 18874368
      };
    }
    if (cmd === 'get_app_version') {
      return '0.1.0';
    }
    return null;
  };
  
  const listen = async (event, callback) => {
    console.log(`Mock listen: ${event}`);
    return () => {}; // Return unlisten function
  };
  
  const open = async (options) => {
    console.log('Mock file dialog:', options);
    if (options.directory) {
      return 'C:\\Users\\Demo\\RecoveredFiles';
    }
    return 'C:\\Users\\Demo\\disk_image.img';
  };
  
  let activeSection = 'recovery';
  let selectedFile = '';
  let selectedOutput = '';
  let filesystemType = 'xfs';
  let confidenceThreshold = 0.7;
  let isScanning = false;
  let isRecovering = false;
  let scanProgress = { progress: 0, message: '', files_found: 0, recoverable_files: 0 };
  let sessionFiles = [];
  let currentSessionId = '';
  let recoveryHistory = [];
  let appVersion = '';

  // Navigation
  function setActiveSection(section) {
    activeSection = section;
  }

  // File selection
  async function selectImageFile() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Disk Images',
          extensions: ['img', 'iso', 'dd', 'raw', 'bin']
        }]
      });
      if (selected) {
        selectedFile = selected;
        await detectFilesystem();
      }
    } catch (error) {
      console.error('Error selecting file:', error);
    }
  }

  async function selectOutputFolder() {
    try {
      const selected = await open({
        directory: true
      });
      if (selected) {
        selectedOutput = selected;
      }
    } catch (error) {
      console.error('Error selecting output folder:', error);
    }
  }

  // Filesystem detection
  async function detectFilesystem() {
    if (!selectedFile) return;
    
    try {
      const detected = await invoke('detect_filesystem', { imagePath: selectedFile });
      filesystemType = detected.toLowerCase();
    } catch (error) {
      console.error('Filesystem detection failed:', error);
    }
  }

  // Scanning
  async function startScan() {
    if (!selectedFile) {
      alert('Please select a disk image file first');
      return;
    }

    isScanning = true;
    scanProgress = { progress: 0, message: 'Initializing...', files_found: 0, recoverable_files: 0 };

    try {
      const sessionInfo = await invoke('start_scan', {
        imagePath: selectedFile,
        fsType: filesystemType,
        confidence: confidenceThreshold
      });

      currentSessionId = sessionInfo.id;
      await loadSessionFiles();
      
      // Add to history
      recoveryHistory.unshift({
        id: sessionInfo.id,
        date: new Date(sessionInfo.created_at).toLocaleDateString(),
        device: sessionInfo.device_path,
        filesFound: sessionInfo.files_found,
        recoverable: sessionInfo.recoverable_files,
        fsType: sessionInfo.fs_type
      });

    } catch (error) {
      console.error('Scan failed:', error);
      alert(`Scan failed: ${error}`);
    } finally {
      isScanning = false;
    }
  }

  async function loadSessionFiles() {
    if (!currentSessionId) return;

    try {
      sessionFiles = await invoke('get_session_files', { sessionId: currentSessionId });
    } catch (error) {
      console.error('Failed to load session files:', error);
    }
  }

  // Recovery
  async function startRecovery() {
    if (!currentSessionId || !selectedOutput) {
      alert('Please complete a scan and select an output folder first');
      return;
    }

    const selectedFileIds = sessionFiles
      .filter(file => file.selected)
      .map(file => file.id);

    if (selectedFileIds.length === 0) {
      alert('Please select at least one file to recover');
      return;
    }

    isRecovering = true;

    try {
      const result = await invoke('recover_session_files', {
        sessionId: currentSessionId,
        outputDir: selectedOutput,
        fileIds: selectedFileIds.length > 0 ? selectedFileIds : null
      });

      alert(`Recovery completed! ${result.files_recovered} files recovered.`);
    } catch (error) {
      console.error('Recovery failed:', error);
      alert(`Recovery failed: ${error}`);
    } finally {
      isRecovering = false;
    }
  }

  function toggleFileSelection(fileId) {
    sessionFiles = sessionFiles.map(file => 
      file.id === fileId ? { ...file, selected: !file.selected } : file
    );
  }

  function selectAllFiles() {
    const allSelected = sessionFiles.every(file => file.selected);
    sessionFiles = sessionFiles.map(file => ({ ...file, selected: !allSelected }));
  }

  function formatFileSize(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  function getStatusClass(confidence) {
    if (confidence >= 0.8) return 'recoverable';
    if (confidence >= 0.5) return 'uncertain';
    return 'damaged';
  }

  function getStatusText(confidence) {
    if (confidence >= 0.8) return 'Recoverable';
    if (confidence >= 0.5) return 'Uncertain';
    return 'Damaged';
  }

  onMount(async () => {
    // Get app version
    try {
      appVersion = await invoke('get_app_version');
    } catch (error) {
      console.error('Failed to get app version:', error);
    }

    // Listen for scan progress updates
    const unlisten = await listen('scan-progress', (event) => {
      scanProgress = event.payload;
    });

    return () => {
      unlisten();
    };
  });
</script>

<div class="app-container">
  <!-- Sidebar Navigation -->
  <nav class="sidebar">
    <button 
      class="nav-item" 
      class:active={activeSection === 'recovery'}
      on:click={() => setActiveSection('recovery')}
    >
      🔍 Recovery
    </button>
    
    <button 
      class="nav-item" 
      class:active={activeSection === 'account'}
      on:click={() => setActiveSection('account')}
    >
      👤 Account
    </button>
    
    <button 
      class="nav-item" 
      class:active={activeSection === 'history'}
      on:click={() => setActiveSection('history')}
    >
      📊 History
    </button>
    
    <button 
      class="nav-item" 
      class:active={activeSection === 'settings'}
      on:click={() => setActiveSection('settings')}
    >
      ⚙️ Settings
    </button>
  </nav>

  <!-- Main Content -->
  <main class="main-content">
    <!-- Recovery Section -->
    <div class="section" class:active={activeSection === 'recovery'}>
      <h2>Data Recovery</h2>
      
      <!-- File Selection -->
      <div class="card">
        <h3>1. Select Disk Image</h3>
        <div class="form-group">
          <label>Disk Image File:</label>
          <div class="flex gap-10">
            <input 
              type="text" 
              class="form-control" 
              bind:value={selectedFile} 
              placeholder="Select a disk image file..."
              readonly
            />
            <button class="btn btn-secondary" on:click={selectImageFile}>
              Browse
            </button>
          </div>
        </div>
        
        <div class="form-group">
          <label>Filesystem Type:</label>
          <select class="form-control" bind:value={filesystemType}>
            <option value="xfs">XFS</option>
            <option value="btrfs">Btrfs</option>
            <option value="exfat">exFAT</option>
          </select>
        </div>
      </div>

      <!-- Scan Configuration -->
      <div class="card">
        <h3>2. Scan Configuration</h3>
        <div class="form-group">
          <label>Confidence Threshold: {Math.round(confidenceThreshold * 100)}%</label>
          <input 
            type="range" 
            class="slider" 
            min="0.1" 
            max="1.0" 
            step="0.1" 
            bind:value={confidenceThreshold}
          />
          <small class="text-muted">Higher values show only files with better recovery chances</small>
        </div>
        
        <button 
          class="btn btn-primary" 
          on:click={startScan}
          disabled={!selectedFile || isScanning}
        >
          {isScanning ? 'Scanning...' : 'Start Scan'}
        </button>
      </div>

      <!-- Scan Progress -->
      {#if isScanning}
        <div class="card">
          <h3>Scan Progress</h3>
          <div class="progress-container">
            <div class="progress-bar" style="width: {scanProgress.progress}%"></div>
          </div>
          <div class="progress-text">
            {scanProgress.message} - {Math.round(scanProgress.progress)}%
          </div>
          <div class="flex flex-between mt-20">
            <span>Files Found: {scanProgress.files_found}</span>
            <span>Recoverable: {scanProgress.recoverable_files}</span>
          </div>
        </div>
      {/if}

      <!-- Scan Results -->
      {#if sessionFiles.length > 0}
        <div class="card">
          <div class="flex flex-between">
            <h3>Scan Results ({sessionFiles.length} files found)</h3>
            <div class="flex gap-10">
              <button class="btn btn-secondary" on:click={selectAllFiles}>
                Toggle All
              </button>
              <button class="btn btn-secondary" on:click={selectOutputFolder}>
                Select Output Folder
              </button>
              <button 
                class="btn btn-success" 
                on:click={startRecovery}
                disabled={!selectedOutput || isRecovering}
              >
                {isRecovering ? 'Recovering...' : 'Recover Selected'}
              </button>
            </div>
          </div>
          
          {#if selectedOutput}
            <p class="text-muted mb-20">Output: {selectedOutput}</p>
          {/if}

          <div class="table-container">
            <table class="table">
              <thead>
                <tr>
                  <th>Select</th>
                  <th>Name</th>
                  <th>Size</th>
                  <th>Type</th>
                  <th>Confidence</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {#each sessionFiles as file}
                  <tr>
                    <td>
                      <input 
                        type="checkbox" 
                        bind:checked={file.selected}
                        on:change={() => toggleFileSelection(file.id)}
                      />
                    </td>
                    <td>{file.name}</td>
                    <td>{formatFileSize(file.size)}</td>
                    <td>{file.type}</td>
                    <td>{Math.round(file.confidence * 100)}%</td>
                    <td>
                      <span class="status {getStatusClass(file.confidence)}">
                        {getStatusText(file.confidence)}
                      </span>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      {/if}
    </div>

    <!-- Account Section -->
    <div class="section" class:active={activeSection === 'account'}>
      <h2>Account</h2>
      
      <div class="card">
        <h3>User Information</h3>
        <div class="form-group">
          <label>License Type:</label>
          <p>Professional Edition</p>
        </div>
        <div class="form-group">
          <label>License Status:</label>
          <p class="text-success">Active</p>
        </div>
        <div class="form-group">
          <label>Version:</label>
          <p>{appVersion}</p>
        </div>
      </div>

      <div class="card">
        <h3>Features</h3>
        <ul>
          <li>✅ XFS File System Recovery</li>
          <li>✅ Btrfs File System Recovery</li>
          <li>✅ exFAT File System Recovery</li>
          <li>✅ Advanced Signature Analysis</li>
          <li>✅ Confidence Scoring</li>
          <li>✅ Batch Recovery</li>
          <li>✅ Recovery History</li>
        </ul>
      </div>
    </div>

    <!-- History Section -->
    <div class="section" class:active={activeSection === 'history'}>
      <h2>Recovery History</h2>
      
      {#if recoveryHistory.length > 0}
        <div class="table-container">
          <table class="table">
            <thead>
              <tr>
                <th>Date</th>
                <th>Device</th>
                <th>Filesystem</th>
                <th>Files Found</th>
                <th>Recoverable</th>
                <th>Session ID</th>
              </tr>
            </thead>
            <tbody>
              {#each recoveryHistory as session}
                <tr>
                  <td>{session.date}</td>
                  <td>{session.device}</td>
                  <td>{session.fsType}</td>
                  <td>{session.filesFound}</td>
                  <td>{session.recoverable}</td>
                  <td class="text-muted">{session.id.substring(0, 8)}...</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <div class="card text-center">
          <p class="text-muted">No recovery sessions yet. Start a scan to see your history here.</p>
        </div>
      {/if}
    </div>

    <!-- Settings Section -->
    <div class="section" class:active={activeSection === 'settings'}>
      <h2>Settings</h2>
      
      <div class="card">
        <h3>Recovery Settings</h3>
        <div class="form-group">
          <label>Default Confidence Threshold:</label>
          <input type="range" class="slider" min="0.1" max="1.0" step="0.1" value="0.7" />
        </div>
        <div class="form-group">
          <label>
            <input type="checkbox" checked /> Enable detailed logging
          </label>
        </div>
        <div class="form-group">
          <label>
            <input type="checkbox" checked /> Auto-save recovery sessions
          </label>
        </div>
      </div>

      <div class="card">
        <h3>Interface</h3>
        <div class="form-group">
          <label>Theme:</label>
          <select class="form-control">
            <option>Dark (Default)</option>
            <option>Light</option>
            <option>Auto</option>
          </select>
        </div>
      </div>
    </div>
  </main>
</div>
