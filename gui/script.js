// GhostFS GUI JavaScript
class GhostFSApp {
    constructor() {
        this.currentSection = 'recovery';
        this.isScanning = false;
        this.scanProgress = 0;
        this.foundFiles = [];
        
        this.initializeEventListeners();
        this.initializeSliders();
    }

    initializeEventListeners() {
        // Navigation
        document.querySelectorAll('.nav-item').forEach(item => {
            item.addEventListener('click', (e) => {
                const section = e.currentTarget.dataset.section;
                this.switchSection(section);
            });
        });

        // File input
        const fileInput = document.getElementById('file-input');
        fileInput.addEventListener('change', (e) => {
            this.handleFileSelection(e.target.files[0]);
        });

        // Start scan button
        document.getElementById('start-scan-btn').addEventListener('click', () => {
            this.startScan();
        });

        // Recover button
        document.getElementById('recover-btn').addEventListener('click', () => {
            this.recoverSelectedFiles();
        });

        // Select all checkbox
        document.getElementById('select-all').addEventListener('change', (e) => {
            this.toggleSelectAll(e.target.checked);
        });

        // Confidence slider
        document.getElementById('confidence-slider').addEventListener('input', (e) => {
            document.getElementById('confidence-value').textContent = e.target.value + '%';
        });
    }

    initializeSliders() {
        // Initialize confidence slider
        const confidenceSlider = document.getElementById('confidence-slider');
        const confidenceValue = document.getElementById('confidence-value');
        confidenceValue.textContent = confidenceSlider.value + '%';
    }

    switchSection(sectionName) {
        // Update navigation
        document.querySelectorAll('.nav-item').forEach(item => {
            item.classList.remove('active');
        });
        document.querySelector(`[data-section="${sectionName}"]`).classList.add('active');

        // Update content sections
        document.querySelectorAll('.content-section').forEach(section => {
            section.classList.remove('active');
        });
        document.getElementById(`${sectionName}-section`).classList.add('active');

        this.currentSection = sectionName;

        // Show history when not in recovery mode and not scanning
        if (sectionName !== 'recovery' && !this.isScanning) {
            this.showHistoryInSidebar();
        }
    }

    handleFileSelection(file) {
        if (file) {
            const selectedFileDiv = document.getElementById('selected-file');
            selectedFileDiv.textContent = `Selected: ${file.name} (${this.formatFileSize(file.size)})`;
            selectedFileDiv.style.display = 'block';
            
            // Enable start scan button
            document.getElementById('start-scan-btn').disabled = false;
        }
    }

    async startScan() {
        if (this.isScanning) return;

        const fileInput = document.getElementById('file-input');
        const fsType = document.getElementById('fs-type').value;
        const confidence = document.getElementById('confidence-slider').value / 100;

        if (!fileInput.files[0]) {
            alert('Please select a disk image file first.');
            return;
        }

        this.isScanning = true;
        this.showProgressPanel();
        this.updateScanProgress(0, 'Initializing scan...');

        try {
            // Simulate scanning process
            await this.simulateScan(fileInput.files[0], fsType, confidence);
        } catch (error) {
            console.error('Scan failed:', error);
            this.showError('Scan failed: ' + error.message);
        } finally {
            this.isScanning = false;
        }
    }

    async simulateScan(file, fsType, confidence) {
        const stages = [
            { progress: 10, message: 'Reading filesystem superblock...' },
            { progress: 25, message: 'Analyzing allocation groups...' },
            { progress: 40, message: 'Scanning for deleted inodes...' },
            { progress: 60, message: 'Performing signature analysis...' },
            { progress: 80, message: 'Calculating confidence scores...' },
            { progress: 95, message: 'Finalizing results...' },
            { progress: 100, message: 'Scan complete!' }
        ];

        for (const stage of stages) {
            await this.delay(800);
            this.updateScanProgress(stage.progress, stage.message);
            
            // Update scan details
            const filesFound = Math.floor(stage.progress / 10);
            const recoverableFiles = Math.floor(filesFound * confidence);
            
            document.getElementById('files-found').textContent = filesFound;
            document.getElementById('recoverable-files').textContent = recoverableFiles;
            document.getElementById('scan-status').textContent = 
                stage.progress === 100 ? 'Complete' : 'Scanning';
        }

        // Generate mock results
        this.generateMockResults(confidence);
        this.showResults();
    }

    generateMockResults(confidence) {
        const mockFiles = [
            { name: 'vacation_photo.jpg', size: 2048000, type: 'image', confidence: 0.95 },
            { name: 'document.pdf', size: 512000, type: 'document', confidence: 0.78 },
            { name: 'video_clip.mp4', size: 15728640, type: 'video', confidence: 0.45 },
            { name: 'archive.zip', size: 1024000, type: 'archive', confidence: 0.62 },
            { name: 'presentation.pptx', size: 3145728, type: 'document', confidence: 0.89 },
            { name: 'music.mp3', size: 4194304, type: 'audio', confidence: 0.71 }
        ];

        this.foundFiles = mockFiles.filter(file => file.confidence >= confidence);
        this.updateResultsTable();
    }

    updateResultsTable() {
        const tbody = document.getElementById('files-table-body');
        tbody.innerHTML = '';

        this.foundFiles.forEach((file, index) => {
            const row = document.createElement('tr');
            const confidenceClass = this.getConfidenceClass(file.confidence);
            
            row.innerHTML = `
                <td><input type="checkbox" data-file-index="${index}" checked></td>
                <td>${file.name}</td>
                <td>${this.formatFileSize(file.size)}</td>
                <td><span class="confidence-badge ${confidenceClass}">${Math.round(file.confidence * 100)}%</span></td>
            `;
            
            tbody.appendChild(row);
        });
    }

    getConfidenceClass(confidence) {
        if (confidence >= 0.8) return 'high';
        if (confidence >= 0.5) return 'medium';
        return 'low';
    }

    showProgressPanel() {
        document.getElementById('progress-panel').style.display = 'block';
        document.getElementById('start-scan-btn').disabled = true;
        document.getElementById('start-scan-btn').innerHTML = '<i class="fas fa-spinner fa-spin"></i> Scanning...';
    }

    updateScanProgress(progress, message) {
        document.getElementById('progress-fill').style.width = progress + '%';
        document.getElementById('progress-text').textContent = message;
    }

    showResults() {
        document.getElementById('start-scan-btn').disabled = false;
        document.getElementById('start-scan-btn').innerHTML = '<i class="fas fa-play"></i> Start Scan';
        
        // Update confidence display
        const avgConfidence = this.foundFiles.reduce((sum, file) => sum + file.confidence, 0) / this.foundFiles.length;
        document.querySelector('.confidence-fill').style.width = (avgConfidence * 100) + '%';
        document.querySelector('.confidence-percent').textContent = Math.round(avgConfidence * 100) + '%';
    }

    async recoverSelectedFiles() {
        const selectedCheckboxes = document.querySelectorAll('#files-table-body input[type="checkbox"]:checked');
        const selectedFiles = Array.from(selectedCheckboxes).map(cb => 
            this.foundFiles[parseInt(cb.dataset.fileIndex)]
        );

        if (selectedFiles.length === 0) {
            alert('Please select files to recover.');
            return;
        }

        // Simulate recovery process
        const recoverBtn = document.getElementById('recover-btn');
        recoverBtn.disabled = true;
        recoverBtn.innerHTML = '<i class="fas fa-spinner fa-spin"></i> Recovering...';

        try {
            await this.simulateRecovery(selectedFiles);
            
            // Add to history
            this.addToHistory({
                type: document.getElementById('fs-type').value.toUpperCase(),
                filesRecovered: selectedFiles.length,
                totalSize: selectedFiles.reduce((sum, file) => sum + file.size, 0),
                confidence: Math.round(selectedFiles.reduce((sum, file) => sum + file.confidence, 0) / selectedFiles.length * 100),
                timestamp: new Date(),
                status: 'success'
            });

            alert(`Successfully recovered ${selectedFiles.length} files!`);
        } catch (error) {
            console.error('Recovery failed:', error);
            alert('Recovery failed: ' + error.message);
        } finally {
            recoverBtn.disabled = false;
            recoverBtn.innerHTML = '<i class="fas fa-download"></i> Recover Selected';
        }
    }

    async simulateRecovery(files) {
        // Simulate recovery time based on file sizes
        const totalSize = files.reduce((sum, file) => sum + file.size, 0);
        const recoveryTime = Math.min(3000, totalSize / 1000000 * 1000); // Max 3 seconds
        
        await this.delay(recoveryTime);
        
        // In a real implementation, this would call the GhostFS CLI
        console.log('Recovering files:', files);
    }

    toggleSelectAll(checked) {
        document.querySelectorAll('#files-table-body input[type="checkbox"]').forEach(cb => {
            cb.checked = checked;
        });
    }

    addToHistory(session) {
        // In a real implementation, this would save to localStorage or backend
        console.log('Adding to history:', session);
        
        // Update history display if currently viewing history
        if (this.currentSection === 'history') {
            this.refreshHistoryDisplay();
        }
    }

    refreshHistoryDisplay() {
        // This would fetch and display actual history data
        console.log('Refreshing history display');
    }

    showHistoryInSidebar() {
        // Show history when not in recovery mode
        console.log('Showing history in sidebar for section:', this.currentSection);
    }

    showError(message) {
        // Create and show error notification
        const errorDiv = document.createElement('div');
        errorDiv.className = 'error-notification';
        errorDiv.innerHTML = `
            <i class="fas fa-exclamation-triangle"></i>
            <span>${message}</span>
            <button onclick="this.parentElement.remove()">×</button>
        `;
        
        document.body.appendChild(errorDiv);
        
        // Auto-remove after 5 seconds
        setTimeout(() => {
            if (errorDiv.parentElement) {
                errorDiv.remove();
            }
        }, 5000);
    }

    formatFileSize(bytes) {
        if (bytes === 0) return '0 B';
        
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
    }

    delay(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
}

// Initialize the application when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.ghostfsApp = new GhostFSApp();
});

// Backend Integration Functions (for future implementation)
class GhostFSBackend {
    static async detectFilesystem(file) {
        // This would call the GhostFS CLI detect command
        const formData = new FormData();
        formData.append('image', file);
        
        try {
            const response = await fetch('/api/detect', {
                method: 'POST',
                body: formData
            });
            return await response.json();
        } catch (error) {
            console.error('Detection failed:', error);
            throw error;
        }
    }

    static async scanImage(file, fsType, confidence) {
        // This would call the GhostFS CLI scan command
        const formData = new FormData();
        formData.append('image', file);
        formData.append('fs', fsType);
        formData.append('confidence', confidence);
        
        try {
            const response = await fetch('/api/scan', {
                method: 'POST',
                body: formData
            });
            return await response.json();
        } catch (error) {
            console.error('Scan failed:', error);
            throw error;
        }
    }

    static async recoverFiles(sessionId, fileIds, outputDir) {
        // This would call the GhostFS CLI recover command
        try {
            const response = await fetch('/api/recover', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    sessionId,
                    fileIds,
                    outputDir
                })
            });
            return await response.json();
        } catch (error) {
            console.error('Recovery failed:', error);
            throw error;
        }
    }

    static async getHistory() {
        // This would fetch recovery history from backend
        try {
            const response = await fetch('/api/history');
            return await response.json();
        } catch (error) {
            console.error('Failed to fetch history:', error);
            throw error;
        }
    }
}
