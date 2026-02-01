// GhostFS GUI JavaScript - Redesigned Interface
class GhostFSApp {
    constructor() {
        this.isScanning = false;
        this.scanProgress = 0;
        this.foundFiles = [];
        this.selectedFiles = [];
        this.scanStartTime = null;
        this.scanTimer = null;
        
        // User data
        this.userData = {
            name: 'John Doe',
            email: 'john.doe@example.com',
            planType: 'Professional',
            planExpiry: 'Dec 31, 2025',
            recoveriesUsed: 12,
            recoveriesTotal: 50,
            totalRecoveries: 247,
            dataRecovered: '15.2 GB',
            successRate: '89%'
        };
        
        this.initializeEventListeners();
        this.initializeUserInterface();
        this.setupDragAndDrop();
    }

    initializeEventListeners() {
        // File input
        const fileInput = document.getElementById('file-input');
        if (fileInput) {
            fileInput.addEventListener('change', (e) => {
                this.handleFileSelection(e.target.files[0]);
            });
        }

        // Remove file button
        const removeFileBtn = document.getElementById('remove-file');
        if (removeFileBtn) {
            removeFileBtn.addEventListener('click', () => {
                this.clearSelectedFile();
            });
        }

        // Start recovery button
        const startRecoveryBtn = document.getElementById('start-recovery-btn');
        if (startRecoveryBtn) {
            startRecoveryBtn.addEventListener('click', () => {
                this.startRecovery();
            });
        }

        // Cancel recovery button
        const cancelBtn = document.getElementById('cancel-recovery');
        if (cancelBtn) {
            cancelBtn.addEventListener('click', () => {
                this.cancelRecovery();
            });
        }

        // Recover files button
        const recoverBtn = document.getElementById('recover-files');
        if (recoverBtn) {
            recoverBtn.addEventListener('click', () => {
                this.recoverSelectedFiles();
            });
        }

        // Select all files checkbox
        const selectAllBtn = document.getElementById('select-all-files');
        if (selectAllBtn) {
            selectAllBtn.addEventListener('change', (e) => {
                this.toggleSelectAll(e.target.checked);
            });
        }

        // Confidence slider
        const confidenceSlider = document.getElementById('confidence-slider');
        const confidenceValue = document.getElementById('confidence-value');
        if (confidenceSlider && confidenceValue) {
            confidenceSlider.addEventListener('input', (e) => {
                confidenceValue.textContent = e.target.value + '%';
            });
        }

        // Confidence filter slider
        const confidenceFilter = document.getElementById('confidence-filter');
        const confidenceFilterValue = document.getElementById('confidence-filter-value');
        if (confidenceFilter && confidenceFilterValue) {
            confidenceFilter.addEventListener('input', (e) => {
                confidenceFilterValue.textContent = e.target.value + '%';
                this.filterResults();
            });
        }

        // Filter checkboxes
        document.querySelectorAll('[data-filter]').forEach(checkbox => {
            checkbox.addEventListener('change', () => {
                this.filterResults();
            });
        });

        // Size filters
        const minSize = document.getElementById('min-size');
        const maxSize = document.getElementById('max-size');
        if (minSize && maxSize) {
            minSize.addEventListener('input', () => this.filterResults());
            maxSize.addEventListener('input', () => this.filterResults());
        }

        // Sidebar action buttons
        const accountBtn = document.getElementById('account-settings');
        const historyBtn = document.getElementById('view-history');
        const upgradeBtn = document.getElementById('upgrade-plan');

        if (accountBtn) {
            accountBtn.addEventListener('click', () => this.showAccountSettings());
        }
        if (historyBtn) {
            historyBtn.addEventListener('click', () => this.showHistory());
        }
        if (upgradeBtn) {
            upgradeBtn.addEventListener('click', () => this.showUpgradeOptions());
        }

        // Advanced options toggle
        const advancedBtn = document.getElementById('advanced-options-btn');
        if (advancedBtn) {
            advancedBtn.addEventListener('click', () => this.toggleAdvancedOptions());
        }
    }

    initializeUserInterface() {
        // Populate user data
        this.updateUserInterface();
        
        // Initialize confidence slider
        const confidenceSlider = document.getElementById('confidence-slider');
        const confidenceValue = document.getElementById('confidence-value');
        if (confidenceSlider && confidenceValue) {
            confidenceValue.textContent = confidenceSlider.value + '%';
        }

        // Initialize confidence filter
        const confidenceFilter = document.getElementById('confidence-filter');
        const confidenceFilterValue = document.getElementById('confidence-filter-value');
        if (confidenceFilter && confidenceFilterValue) {
            confidenceFilterValue.textContent = confidenceFilter.value + '%';
        }
    }

    updateUserInterface() {
        // Update user profile
        const userName = document.getElementById('user-name');
        const userEmail = document.getElementById('user-email');
        const planType = document.getElementById('plan-type');
        const planExpiry = document.getElementById('plan-expiry');
        const recoveriesUsed = document.getElementById('recoveries-used');
        const usageFill = document.getElementById('usage-fill');

        if (userName) userName.textContent = this.userData.name;
        if (userEmail) userEmail.textContent = this.userData.email;
        if (planType) planType.textContent = this.userData.planType;
        if (planExpiry) planExpiry.textContent = this.userData.planExpiry;
        if (recoveriesUsed) {
            recoveriesUsed.textContent = `${this.userData.recoveriesUsed} / ${this.userData.recoveriesTotal}`;
        }
        if (usageFill) {
            const percentage = (this.userData.recoveriesUsed / this.userData.recoveriesTotal) * 100;
            usageFill.style.width = percentage + '%';
        }

        // Update quick stats
        const statElements = document.querySelectorAll('.stat-number');
        if (statElements.length >= 3) {
            statElements[0].textContent = this.userData.totalRecoveries;
            statElements[1].textContent = this.userData.dataRecovered;
            statElements[2].textContent = this.userData.successRate;
        }
    }

    setupDragAndDrop() {
        const dropZone = document.getElementById('file-drop-zone');
        if (!dropZone) return;

        // Prevent default drag behaviors
        ['dragenter', 'dragover', 'dragleave', 'drop'].forEach(eventName => {
            dropZone.addEventListener(eventName, this.preventDefaults, false);
            document.body.addEventListener(eventName, this.preventDefaults, false);
        });

        // Highlight drop zone when item is dragged over it
        ['dragenter', 'dragover'].forEach(eventName => {
            dropZone.addEventListener(eventName, () => {
                dropZone.classList.add('dragover');
            }, false);
        });

        ['dragleave', 'drop'].forEach(eventName => {
            dropZone.addEventListener(eventName, () => {
                dropZone.classList.remove('dragover');
            }, false);
        });

        // Handle dropped files
        dropZone.addEventListener('drop', (e) => {
            const files = e.dataTransfer.files;
            if (files.length > 0) {
                this.handleFileSelection(files[0]);
            }
        }, false);
    }

    preventDefaults(e) {
        e.preventDefault();
        e.stopPropagation();
    }

    handleFileSelection(file) {
        if (!file) return;

        // Check file type
        const allowedTypes = ['.img', '.iso', '.dd', '.raw'];
        const fileExtension = '.' + file.name.split('.').pop().toLowerCase();
        
        if (!allowedTypes.includes(fileExtension)) {
            this.showNotification('Unsupported file format. Please select a .img, .iso, .dd, or .raw file.', 'error');
            return;
        }

        // Update UI
        const selectedFileInfo = document.getElementById('selected-file-info');
        const selectedFileName = document.getElementById('selected-file-name');
        const selectedFileSize = document.getElementById('selected-file-size');
        const dropZone = document.getElementById('file-drop-zone');
        const startBtn = document.getElementById('start-recovery-btn');

        if (selectedFileInfo && selectedFileName && selectedFileSize && dropZone && startBtn) {
            selectedFileName.textContent = file.name;
            selectedFileSize.textContent = this.formatFileSize(file.size);
            
            // Hide drop zone content, show file info
            dropZone.querySelector('.drop-zone-content').style.display = 'none';
            selectedFileInfo.style.display = 'block';
            
            // Enable start button
            startBtn.disabled = false;
            
            this.showNotification(`Selected: ${file.name}`, 'success');
        }
    }

    clearSelectedFile() {
        const selectedFileInfo = document.getElementById('selected-file-info');
        const dropZone = document.getElementById('file-drop-zone');
        const fileInput = document.getElementById('file-input');
        const startBtn = document.getElementById('start-recovery-btn');

        if (selectedFileInfo && dropZone && fileInput && startBtn) {
            // Reset file input
            fileInput.value = '';
            
            // Show drop zone content, hide file info
            dropZone.querySelector('.drop-zone-content').style.display = 'block';
            selectedFileInfo.style.display = 'none';
            
            // Disable start button
            startBtn.disabled = true;
            
            this.showNotification('File selection cleared', 'info');
        }
    }

    async startRecovery() {
        if (this.isScanning) return;

        const fileInput = document.getElementById('file-input');
        const fsType = document.getElementById('fs-type').value;
        const confidence = document.getElementById('confidence-slider').value / 100;

        if (!fileInput.files[0]) {
            this.showNotification('Please select a disk image file first.', 'error');
            return;
        }

        this.isScanning = true;
        this.scanStartTime = Date.now();
        this.showProgressSection();
        this.updateScanProgress(0, 'Initializing recovery scan...');
        this.startTimer();

        try {
            // Simulate scanning process
            await this.simulateRecoveryScan(fileInput.files[0], fsType, confidence);
        } catch (error) {
            console.error('Recovery scan failed:', error);
            this.showNotification('Recovery scan failed: ' + error.message, 'error');
        } finally {
            this.isScanning = false;
            this.stopTimer();
        }
    }

    async simulateRecoveryScan(file, fsType, confidence) {
        const stages = [
            { progress: 5, message: 'Reading disk image header...', files: 0, recoverable: 0, size: 0 },
            { progress: 15, message: 'Analyzing filesystem structure...', files: 2, recoverable: 1, size: 512 },
            { progress: 25, message: 'Scanning allocation tables...', files: 8, recoverable: 6, size: 2048 },
            { progress: 35, message: 'Identifying deleted inodes...', files: 15, recoverable: 12, size: 5120 },
            { progress: 50, message: 'Performing signature analysis...', files: 28, recoverable: 22, size: 12288 },
            { progress: 65, message: 'Analyzing file fragments...', files: 42, recoverable: 35, size: 20480 },
            { progress: 80, message: 'Calculating confidence scores...', files: 58, recoverable: 48, size: 32768 },
            { progress: 95, message: 'Finalizing recovery results...', files: 67, recoverable: 52, size: 41943 },
            { progress: 100, message: 'Recovery scan complete!', files: 73, recoverable: 56, size: 45320 }
        ];

        for (const stage of stages) {
            await this.delay(900);
            this.updateScanProgress(
                stage.progress, 
                stage.message,
                stage.files,
                stage.recoverable,
                stage.size
            );
        }

        // Generate mock results
        this.generateMockResults(confidence);
        this.showResults();
    }

    updateScanProgress(progress, message, filesFound = 0, recoverableFiles = 0, dataSize = 0) {
        const progressFill = document.getElementById('progress-fill');
        const progressText = document.getElementById('progress-text');
        const progressPercentage = document.getElementById('progress-percentage');
        const filesFoundEl = document.getElementById('files-found');
        const recoverableEl = document.getElementById('recoverable-files');
        const dataSizeEl = document.getElementById('data-size');

        if (progressFill) progressFill.style.width = progress + '%';
        if (progressText) progressText.textContent = message;
        if (progressPercentage) progressPercentage.textContent = progress + '%';
        if (filesFoundEl) filesFoundEl.textContent = filesFound;
        if (recoverableEl) recoverableEl.textContent = recoverableFiles;
        if (dataSizeEl) dataSizeEl.textContent = this.formatFileSize(dataSize * 1024);
    }

    generateMockResults(confidence) {
        const mockFiles = [
            { name: 'family_vacation.jpg', size: 2048000, type: 'images', confidence: 0.98, extension: 'jpg' },
            { name: 'work_presentation.pptx', size: 1536000, type: 'documents', confidence: 0.85, extension: 'pptx' },
            { name: 'wedding_video.mp4', size: 157286400, type: 'videos', confidence: 0.72, extension: 'mp4' },
            { name: 'backup_archive.zip', size: 10240000, type: 'archives', confidence: 0.68, extension: 'zip' },
            { name: 'music_collection.mp3', size: 5120000, type: 'audio', confidence: 0.89, extension: 'mp3' },
            { name: 'project_files.pdf', size: 756000, type: 'documents', confidence: 0.91, extension: 'pdf' },
            { name: 'photo_album.jpg', size: 1800000, type: 'images', confidence: 0.94, extension: 'jpg' },
            { name: 'software_installer.exe', size: 25600000, type: 'archives', confidence: 0.45, extension: 'exe' },
            { name: 'spreadsheet.xlsx', size: 512000, type: 'documents', confidence: 0.82, extension: 'xlsx' },
            { name: 'audio_recording.wav', size: 8192000, type: 'audio', confidence: 0.76, extension: 'wav' },
            { name: 'screenshot.png', size: 1024000, type: 'images', confidence: 0.96, extension: 'png' },
            { name: 'database_backup.sql', size: 3072000, type: 'documents', confidence: 0.67, extension: 'sql' }
        ];

        // Filter by confidence threshold
        this.foundFiles = mockFiles.filter(file => file.confidence >= confidence);
        this.selectedFiles = [];
        
        this.updateResultsTable();
        this.updateResultsSummary();
    }

    updateResultsTable() {
        const tbody = document.getElementById('files-table-body');
        if (!tbody) return;

        tbody.innerHTML = '';

        this.foundFiles.forEach((file, index) => {
            const row = document.createElement('tr');
            const confidenceClass = this.getConfidenceClass(file.confidence);
            const fileIcon = this.getFileIcon(file.type);
            
            row.innerHTML = `
                <td>
                    <label class="checkbox-item">
                        <input type="checkbox" data-file-index="${index}">
                        <span class="checkmark"></span>
                    </label>
                </td>
                <td>
                    <div style="display: flex; align-items: center; gap: 8px;">
                        <i class="fas ${fileIcon}" style="color: #4fd1c7;"></i>
                        ${file.name}
                    </div>
                </td>
                <td>${file.type.charAt(0).toUpperCase() + file.type.slice(1)}</td>
                <td>${this.formatFileSize(file.size)}</td>
                <td><span class="confidence-badge ${confidenceClass}">${Math.round(file.confidence * 100)}%</span></td>
                <td>
                    <button class="view-btn" onclick="ghostfsApp.previewFile(${index})" title="Preview">
                        <i class="fas fa-eye"></i>
                    </button>
                </td>
            `;
            
            tbody.appendChild(row);

            // Add event listener to checkbox
            const checkbox = row.querySelector('input[type="checkbox"]');
            checkbox.addEventListener('change', () => {
                this.updateSelectedFiles();
            });
        });

        this.updateSelectedFiles();
    }

    updateSelectedFiles() {
        const checkboxes = document.querySelectorAll('#files-table-body input[type="checkbox"]');
        this.selectedFiles = Array.from(checkboxes)
            .map((cb, index) => cb.checked ? index : -1)
            .filter(index => index !== -1);

        // Update selected count
        const selectedCount = document.getElementById('selected-count');
        const selectedFilesInfo = document.getElementById('selected-files-info');
        const recoverBtn = document.getElementById('recover-files');

        if (selectedCount) {
            selectedCount.textContent = `${this.selectedFiles.length} files selected`;
        }

        if (selectedFilesInfo) {
            if (this.selectedFiles.length > 0) {
                const totalSize = this.selectedFiles.reduce((sum, index) => 
                    sum + this.foundFiles[index].size, 0);
                selectedFilesInfo.textContent = 
                    `${this.selectedFiles.length} files selected (${this.formatFileSize(totalSize)})`;
            } else {
                selectedFilesInfo.textContent = 'Select files to recover';
            }
        }

        if (recoverBtn) {
            recoverBtn.disabled = this.selectedFiles.length === 0;
        }

        // Update select all checkbox
        const selectAllBtn = document.getElementById('select-all-files');
        if (selectAllBtn) {
            selectAllBtn.checked = this.selectedFiles.length === this.foundFiles.length;
            selectAllBtn.indeterminate = this.selectedFiles.length > 0 && 
                                        this.selectedFiles.length < this.foundFiles.length;
        }
    }

    updateResultsSummary() {
        const totalFound = document.getElementById('total-found');
        const avgConfidence = document.getElementById('avg-confidence');

        if (totalFound) {
            totalFound.textContent = `${this.foundFiles.length} files found`;
        }

        if (avgConfidence && this.foundFiles.length > 0) {
            const average = this.foundFiles.reduce((sum, file) => sum + file.confidence, 0) / this.foundFiles.length;
            avgConfidence.textContent = Math.round(average * 100) + '%';
        }
    }

    filterResults() {
        const minConfidence = document.getElementById('confidence-filter')?.value / 100 || 0;
        const minSize = (document.getElementById('min-size')?.value || 0) * 1024;
        const maxSize = (document.getElementById('max-size')?.value || 1000000) * 1024;
        
        // Get selected file types
        const selectedTypes = Array.from(document.querySelectorAll('[data-filter]:checked'))
            .map(cb => cb.dataset.filter);

        const filteredFiles = this.foundFiles.filter(file => {
            return file.confidence >= minConfidence &&
                   file.size >= minSize &&
                   file.size <= maxSize &&
                   selectedTypes.includes(file.type);
        });

        // Update table with filtered results
        this.displayFilteredResults(filteredFiles);
    }

    displayFilteredResults(filteredFiles) {
        const tbody = document.getElementById('files-table-body');
        if (!tbody) return;

        tbody.innerHTML = '';

        filteredFiles.forEach((file, displayIndex) => {
            const originalIndex = this.foundFiles.indexOf(file);
            const row = document.createElement('tr');
            const confidenceClass = this.getConfidenceClass(file.confidence);
            const fileIcon = this.getFileIcon(file.type);
            
            row.innerHTML = `
                <td>
                    <label class="checkbox-item">
                        <input type="checkbox" data-file-index="${originalIndex}" 
                               ${this.selectedFiles.includes(originalIndex) ? 'checked' : ''}>
                        <span class="checkmark"></span>
                    </label>
                </td>
                <td>
                    <div style="display: flex; align-items: center; gap: 8px;">
                        <i class="fas ${fileIcon}" style="color: #4fd1c7;"></i>
                        ${file.name}
                    </div>
                </td>
                <td>${file.type.charAt(0).toUpperCase() + file.type.slice(1)}</td>
                <td>${this.formatFileSize(file.size)}</td>
                <td><span class="confidence-badge ${confidenceClass}">${Math.round(file.confidence * 100)}%</span></td>
                <td>
                    <button class="view-btn" onclick="ghostfsApp.previewFile(${originalIndex})" title="Preview">
                        <i class="fas fa-eye"></i>
                    </button>
                </td>
            `;
            
            tbody.appendChild(row);

            // Add event listener to checkbox
            const checkbox = row.querySelector('input[type="checkbox"]');
            checkbox.addEventListener('change', () => {
                this.updateSelectedFiles();
            });
        });

        // Update summary for filtered results
        const totalFound = document.getElementById('total-found');
        if (totalFound) {
            totalFound.textContent = `${filteredFiles.length} files shown (${this.foundFiles.length} total found)`;
        }
    }

    toggleSelectAll(checked) {
        const checkboxes = document.querySelectorAll('#files-table-body input[type="checkbox"]');
        checkboxes.forEach(cb => {
            cb.checked = checked;
        });
        this.updateSelectedFiles();
    }

    async recoverSelectedFiles() {
        if (this.selectedFiles.length === 0) {
            this.showNotification('Please select files to recover.', 'warning');
            return;
        }

        const selectedFileObjects = this.selectedFiles.map(index => this.foundFiles[index]);
        const recoverBtn = document.getElementById('recover-files');
        
        // Update button state
        if (recoverBtn) {
            recoverBtn.disabled = true;
            recoverBtn.innerHTML = '<i class="fas fa-spinner fa-spin"></i> Recovering...';
        }

        try {
            await this.simulateRecovery(selectedFileObjects);
            
            // Update user stats
            this.userData.recoveriesUsed += 1;
            this.userData.totalRecoveries += selectedFileObjects.length;
            const totalSize = selectedFileObjects.reduce((sum, file) => sum + file.size, 0);
            this.updateUserInterface();

            this.showNotification(
                `Successfully recovered ${selectedFileObjects.length} files (${this.formatFileSize(totalSize)})!`,
                'success'
            );
            
            // Reset interface
            this.resetInterface();
        } catch (error) {
            console.error('Recovery failed:', error);
            this.showNotification('Recovery failed: ' + error.message, 'error');
        } finally {
            if (recoverBtn) {
                recoverBtn.disabled = false;
                recoverBtn.innerHTML = '<i class="fas fa-download"></i> Recover Selected Files';
            }
        }
    }

    async simulateRecovery(files) {
        // Simulate recovery time based on file sizes
        const totalSize = files.reduce((sum, file) => sum + file.size, 0);
        const recoveryTime = Math.min(4000, Math.max(1000, totalSize / 1000000 * 1000));
        
        await this.delay(recoveryTime);
        
        // In a real implementation, this would call the GhostFS CLI
        console.log('Recovering files:', files);
    }

    cancelRecovery() {
        if (!this.isScanning) return;

        this.isScanning = false;
        this.stopTimer();
        this.hideProgressSection();
        this.showNotification('Recovery scan cancelled', 'info');
    }

    showProgressSection() {
        const diskSelection = document.querySelector('.disk-selection');
        const progressSection = document.getElementById('recovery-progress');
        const resultsSection = document.getElementById('recovery-results');

        if (diskSelection) diskSelection.style.display = 'none';
        if (progressSection) {
            progressSection.style.display = 'block';
            progressSection.classList.add('fade-in');
        }
        if (resultsSection) resultsSection.style.display = 'none';
    }

    hideProgressSection() {
        const diskSelection = document.querySelector('.disk-selection');
        const progressSection = document.getElementById('recovery-progress');

        if (diskSelection) diskSelection.style.display = 'block';
        if (progressSection) progressSection.style.display = 'none';
    }

    showResults() {
        const progressSection = document.getElementById('recovery-progress');
        const resultsSection = document.getElementById('recovery-results');

        if (progressSection) progressSection.style.display = 'none';
        if (resultsSection) {
            resultsSection.style.display = 'block';
            resultsSection.classList.add('fade-in');
        }
    }

    resetInterface() {
        // Clear file selection
        this.clearSelectedFile();
        
        // Hide results
        const resultsSection = document.getElementById('recovery-results');
        if (resultsSection) resultsSection.style.display = 'none';
        
        // Show disk selection
        const diskSelection = document.querySelector('.disk-selection');
        if (diskSelection) diskSelection.style.display = 'block';
        
        // Clear found files
        this.foundFiles = [];
        this.selectedFiles = [];
    }

    startTimer() {
        this.scanTimer = setInterval(() => {
            if (this.scanStartTime) {
                const elapsed = Date.now() - this.scanStartTime;
                const timeElapsed = document.getElementById('time-elapsed');
                if (timeElapsed) {
                    const seconds = Math.floor(elapsed / 1000);
                    const minutes = Math.floor(seconds / 60);
                    const remainingSeconds = seconds % 60;
                    timeElapsed.textContent = 
                        `${minutes.toString().padStart(2, '0')}:${remainingSeconds.toString().padStart(2, '0')}`;
                }
            }
        }, 1000);
    }

    stopTimer() {
        if (this.scanTimer) {
            clearInterval(this.scanTimer);
            this.scanTimer = null;
        }
    }

    getConfidenceClass(confidence) {
        if (confidence >= 0.8) return 'high';
        if (confidence >= 0.5) return 'medium';
        return 'low';
    }

    getFileIcon(type) {
        const icons = {
            'images': 'fa-image',
            'documents': 'fa-file-alt',
            'videos': 'fa-video',
            'audio': 'fa-music',
            'archives': 'fa-file-archive'
        };
        return icons[type] || 'fa-file';
    }

    previewFile(index) {
        const file = this.foundFiles[index];
        if (!file) return;

        // Show preview modal/overlay (simplified for demo)
        this.showNotification(`Preview not available for ${file.name}`, 'info');
    }

    showAccountSettings() {
        this.showNotification('Account settings would open here', 'info');
        // In a real implementation, this would open a settings modal or navigate to settings page
    }

    showHistory() {
        this.showNotification('Recovery history would open here', 'info');
        // In a real implementation, this would show recovery history
    }

    showUpgradeOptions() {
        this.showNotification('Plan upgrade options would open here', 'info');
        // In a real implementation, this would show upgrade options
    }

    toggleAdvancedOptions() {
        this.showNotification('Advanced options would be toggled here', 'info');
        // In a real implementation, this would show/hide advanced configuration options
    }

    showNotification(message, type = 'info') {
        // Create notification element
        const notification = document.createElement('div');
        notification.className = `notification notification-${type}`;
        notification.innerHTML = `
            <div class="notification-content">
                <i class="fas ${this.getNotificationIcon(type)}"></i>
                <span>${message}</span>
                <button class="notification-close" onclick="this.parentElement.parentElement.remove()">
                    <i class="fas fa-times"></i>
                </button>
            </div>
        `;

        // Add styles if not already added
        this.addNotificationStyles();

        // Add to page
        document.body.appendChild(notification);

        // Auto-remove after 5 seconds
        setTimeout(() => {
            if (notification.parentElement) {
                notification.remove();
            }
        }, 5000);

        // Animate in
        setTimeout(() => {
            notification.classList.add('notification-show');
        }, 100);
    }

    getNotificationIcon(type) {
        const icons = {
            'success': 'fa-check-circle',
            'error': 'fa-exclamation-triangle',
            'warning': 'fa-exclamation-circle',
            'info': 'fa-info-circle'
        };
        return icons[type] || 'fa-info-circle';
    }

    addNotificationStyles() {
        if (document.getElementById('notification-styles')) return;

        const styles = document.createElement('style');
        styles.id = 'notification-styles';
        styles.textContent = `
            .notification {
                position: fixed;
                top: 20px;
                right: 20px;
                background: #252a3a;
                border: 1px solid #3a4553;
                border-radius: 8px;
                padding: 16px;
                box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
                z-index: 1000;
                transform: translateX(100%);
                transition: transform 0.3s ease;
                max-width: 400px;
                margin-bottom: 10px;
            }
            .notification-show {
                transform: translateX(0);
            }
            .notification-content {
                display: flex;
                align-items: center;
                gap: 12px;
                color: #e1e5e9;
            }
            .notification-success { border-left: 4px solid #38a169; }
            .notification-error { border-left: 4px solid #e53e3e; }
            .notification-warning { border-left: 4px solid #d69e2e; }
            .notification-info { border-left: 4px solid #4fd1c7; }
            .notification-close {
                background: none;
                border: none;
                color: #a0aec0;
                cursor: pointer;
                padding: 4px;
                margin-left: auto;
            }
            .notification-close:hover {
                color: #e1e5e9;
            }
        `;
        document.head.appendChild(styles);
    }

    formatFileSize(bytes) {
        if (bytes === 0) return '0 B';
        
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
    }

    delay(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
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

    static async getUserData() {
        // This would fetch user account data from backend
        try {
            const response = await fetch('/api/user');
            return await response.json();
        } catch (error) {
            console.error('Failed to fetch user data:', error);
            throw error;
        }
    }

    static async updateUserData(userData) {
        // This would update user account data on backend
        try {
            const response = await fetch('/api/user', {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(userData)
            });
            return await response.json();
        } catch (error) {
            console.error('Failed to update user data:', error);
            throw error;
        }
    }
}
