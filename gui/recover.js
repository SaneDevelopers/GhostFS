// Recovery Page JavaScript
class RecoveryPage {
    constructor() {
        this.selectedFiles = new Set();
        this.allFiles = [];
        this.filteredFiles = [];
        this.currentPage = 1;
        this.filesPerPage = 20;
        this.currentFilter = 'all';
        this.currentSort = 'name';
        this.confidenceFilter = 0;
        
        this.initializeData();
        this.initializeEventListeners();
        this.initializeCharts();
        this.renderFiles();
        this.updateStats();
    }

    initializeData() {
        // Mock recovered files data
        this.allFiles = [
            {
                id: 1,
                name: 'family_vacation_2024.jpg',
                type: 'images',
                size: 2048000,
                confidence: 95,
                recovered: true,
                path: '/recovered/images/family_vacation_2024.jpg',
                dateCreated: '2024-07-15',
                extension: 'jpg'
            },
            {
                id: 2,
                name: 'business_presentation.pptx',
                type: 'documents',
                size: 1536000,
                confidence: 87,
                recovered: true,
                path: '/recovered/documents/business_presentation.pptx',
                dateCreated: '2024-09-10',
                extension: 'pptx'
            },
            {
                id: 3,
                name: 'wedding_ceremony.mp4',
                type: 'videos',
                size: 157286400,
                confidence: 72,
                recovered: true,
                path: '/recovered/videos/wedding_ceremony.mp4',
                dateCreated: '2024-06-20',
                extension: 'mp4'
            },
            {
                id: 4,
                name: 'project_backup.zip',
                type: 'archives',
                size: 10240000,
                confidence: 68,
                recovered: true,
                path: '/recovered/archives/project_backup.zip',
                dateCreated: '2024-08-05',
                extension: 'zip'
            },
            {
                id: 5,
                name: 'favorite_songs.mp3',
                type: 'audio',
                size: 5120000,
                confidence: 89,
                recovered: true,
                path: '/recovered/audio/favorite_songs.mp3',
                dateCreated: '2024-07-30',
                extension: 'mp3'
            },
            {
                id: 6,
                name: 'financial_report.pdf',
                type: 'documents',
                size: 756000,
                confidence: 91,
                recovered: true,
                path: '/recovered/documents/financial_report.pdf',
                dateCreated: '2024-09-01',
                extension: 'pdf'
            },
            {
                id: 7,
                name: 'holiday_photos.jpg',
                type: 'images',
                size: 1800000,
                confidence: 94,
                recovered: true,
                path: '/recovered/images/holiday_photos.jpg',
                dateCreated: '2024-08-15',
                extension: 'jpg'
            },
            {
                id: 8,
                name: 'software_installer.exe',
                type: 'archives',
                size: 25600000,
                confidence: 45,
                recovered: true,
                path: '/recovered/archives/software_installer.exe',
                dateCreated: '2024-05-20',
                extension: 'exe'
            },
            {
                id: 9,
                name: 'budget_spreadsheet.xlsx',
                type: 'documents',
                size: 512000,
                confidence: 82,
                recovered: true,
                path: '/recovered/documents/budget_spreadsheet.xlsx',
                dateCreated: '2024-09-12',
                extension: 'xlsx'
            },
            {
                id: 10,
                name: 'podcast_recording.wav',
                type: 'audio',
                size: 8192000,
                confidence: 76,
                recovered: true,
                path: '/recovered/audio/podcast_recording.wav',
                dateCreated: '2024-08-25',
                extension: 'wav'
            }
        ];

        // Add more mock files to reach 47 total
        for (let i = 11; i <= 47; i++) {
            const types = ['images', 'documents', 'videos', 'audio', 'archives'];
            const type = types[Math.floor(Math.random() * types.length)];
            const extensions = {
                images: ['jpg', 'png', 'gif', 'bmp'],
                documents: ['pdf', 'docx', 'xlsx', 'pptx', 'txt'],
                videos: ['mp4', 'avi', 'mkv', 'mov'],
                audio: ['mp3', 'wav', 'flac', 'ogg'],
                archives: ['zip', 'rar', '7z', 'tar']
            };
            const ext = extensions[type][Math.floor(Math.random() * extensions[type].length)];
            
            this.allFiles.push({
                id: i,
                name: `recovered_file_${i}.${ext}`,
                type: type,
                size: Math.floor(Math.random() * 50000000) + 100000,
                confidence: Math.floor(Math.random() * 60) + 40,
                recovered: true,
                path: `/recovered/${type}/recovered_file_${i}.${ext}`,
                dateCreated: '2024-09-20',
                extension: ext
            });
        }

        this.filteredFiles = [...this.allFiles];
    }

    initializeEventListeners() {
        // Filter tabs
        document.querySelectorAll('.filter-tab').forEach(tab => {
            tab.addEventListener('click', (e) => {
                this.setActiveFilter(e.target.dataset.filter);
                this.filterFiles();
            });
        });

        // Confidence filter
        const confidenceFilter = document.getElementById('confidence-filter');
        if (confidenceFilter) {
            confidenceFilter.addEventListener('change', (e) => {
                this.confidenceFilter = parseInt(e.target.value);
                this.filterFiles();
            });
        }

        // Sort selector
        const sortFiles = document.getElementById('sort-files');
        if (sortFiles) {
            sortFiles.addEventListener('change', (e) => {
                this.currentSort = e.target.value;
                this.sortFiles();
                this.renderFiles();
            });
        }

        // Select all files
        const selectAllBtn = document.getElementById('select-all-files');
        if (selectAllBtn) {
            selectAllBtn.addEventListener('click', () => {
                this.toggleSelectAll();
            });
        }

        // Download selected
        const downloadBtn = document.getElementById('download-selected');
        if (downloadBtn) {
            downloadBtn.addEventListener('click', () => {
                this.downloadSelected();
            });
        }

        // Pagination
        const prevBtn = document.getElementById('prev-page');
        const nextBtn = document.getElementById('next-page');
        if (prevBtn) {
            prevBtn.addEventListener('click', () => {
                if (this.currentPage > 1) {
                    this.currentPage--;
                    this.renderFiles();
                    this.updatePagination();
                }
            });
        }
        if (nextBtn) {
            nextBtn.addEventListener('click', () => {
                const totalPages = Math.ceil(this.filteredFiles.length / this.filesPerPage);
                if (this.currentPage < totalPages) {
                    this.currentPage++;
                    this.renderFiles();
                    this.updatePagination();
                }
            });
        }

        // Section expand/collapse
        document.querySelectorAll('.expand-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                this.toggleSection(e.target.dataset.target);
            });
        });

        // Export report
        const exportBtn = document.querySelector('.export-btn');
        if (exportBtn) {
            exportBtn.addEventListener('click', () => {
                this.exportReport();
            });
        }

        // Save session
        const saveBtn = document.querySelector('.save-session-btn');
        if (saveBtn) {
            saveBtn.addEventListener('click', () => {
                this.saveSession();
            });
        }
    }

    initializeCharts() {
        this.initFileTypeChart();
        this.initConfidenceChart();
    }

    initFileTypeChart() {
        const ctx = document.getElementById('fileTypeChart');
        if (!ctx) return;

        const typeCounts = this.allFiles.reduce((acc, file) => {
            acc[file.type] = (acc[file.type] || 0) + 1;
            return acc;
        }, {});

        const data = {
            labels: Object.keys(typeCounts).map(type => 
                type.charAt(0).toUpperCase() + type.slice(1)
            ),
            datasets: [{
                data: Object.values(typeCounts),
                backgroundColor: [
                    '#4fd1c7',
                    '#38a169',
                    '#d69e2e',
                    '#805ad5',
                    '#e53e3e'
                ],
                borderWidth: 2,
                borderColor: '#252a3a'
            }]
        };

        new Chart(ctx, {
            type: 'doughnut',
            data: data,
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        position: 'bottom',
                        labels: {
                            color: '#a0aec0',
                            font: {
                                size: 12
                            }
                        }
                    }
                }
            }
        });
    }

    initConfidenceChart() {
        const ctx = document.getElementById('confidenceChart');
        if (!ctx) return;

        const confidenceRanges = {
            'High (80-100%)': 0,
            'Medium (60-79%)': 0,
            'Low (40-59%)': 0,
            'Very Low (<40%)': 0
        };

        this.allFiles.forEach(file => {
            if (file.confidence >= 80) confidenceRanges['High (80-100%)']++;
            else if (file.confidence >= 60) confidenceRanges['Medium (60-79%)']++;
            else if (file.confidence >= 40) confidenceRanges['Low (40-59%)']++;
            else confidenceRanges['Very Low (<40%)']++;
        });

        const data = {
            labels: Object.keys(confidenceRanges),
            datasets: [{
                label: 'Files',
                data: Object.values(confidenceRanges),
                backgroundColor: [
                    '#38a169',
                    '#d69e2e',
                    '#e53e3e',
                    '#718096'
                ],
                borderWidth: 2,
                borderColor: '#252a3a'
            }]
        };

        new Chart(ctx, {
            type: 'bar',
            data: data,
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        display: false
                    }
                },
                scales: {
                    x: {
                        ticks: {
                            color: '#a0aec0',
                            font: {
                                size: 10
                            }
                        },
                        grid: {
                            color: '#3a4553'
                        }
                    },
                    y: {
                        ticks: {
                            color: '#a0aec0',
                            font: {
                                size: 10
                            }
                        },
                        grid: {
                            color: '#3a4553'
                        }
                    }
                }
            }
        });
    }

    setActiveFilter(filter) {
        document.querySelectorAll('.filter-tab').forEach(tab => {
            tab.classList.remove('active');
        });
        document.querySelector(`[data-filter="${filter}"]`).classList.add('active');
        this.currentFilter = filter;
    }

    filterFiles() {
        this.filteredFiles = this.allFiles.filter(file => {
            const typeMatch = this.currentFilter === 'all' || file.type === this.currentFilter;
            const confidenceMatch = file.confidence >= this.confidenceFilter;
            return typeMatch && confidenceMatch;
        });

        this.currentPage = 1;
        this.sortFiles();
        this.renderFiles();
        this.updatePagination();
    }

    sortFiles() {
        this.filteredFiles.sort((a, b) => {
            switch (this.currentSort) {
                case 'name':
                    return a.name.localeCompare(b.name);
                case 'size':
                    return b.size - a.size;
                case 'confidence':
                    return b.confidence - a.confidence;
                case 'type':
                    return a.type.localeCompare(b.type);
                default:
                    return 0;
            }
        });
    }

    renderFiles() {
        const filesList = document.getElementById('files-list');
        if (!filesList) return;

        const startIndex = (this.currentPage - 1) * this.filesPerPage;
        const endIndex = startIndex + this.filesPerPage;
        const filesToShow = this.filteredFiles.slice(startIndex, endIndex);

        filesList.innerHTML = '';

        filesToShow.forEach(file => {
            const fileElement = this.createFileElement(file);
            filesList.appendChild(fileElement);
        });

        this.updateSelectedCount();
    }

    createFileElement(file) {
        const fileItem = document.createElement('div');
        fileItem.className = 'file-item fade-in';
        fileItem.dataset.fileId = file.id;

        const isSelected = this.selectedFiles.has(file.id);
        const confidenceClass = this.getConfidenceClass(file.confidence);
        const fileIcon = this.getFileIcon(file.type);

        fileItem.innerHTML = `
            <div class="file-checkbox">
                <input type="checkbox" ${isSelected ? 'checked' : ''} 
                       onchange="recoveryPage.toggleFileSelection(${file.id})">
            </div>
            <div class="file-icon ${file.type}">
                <i class="fas ${fileIcon}"></i>
            </div>
            <div class="file-info">
                <div class="file-name">${file.name}</div>
                <div class="file-details">
                    <span>Size: ${this.formatFileSize(file.size)}</span>
                    <span>Type: ${file.type.charAt(0).toUpperCase() + file.type.slice(1)}</span>
                    <span>Created: ${file.dateCreated}</span>
                </div>
            </div>
            <div class="confidence-indicator">
                <span class="confidence-badge ${confidenceClass}">${file.confidence}%</span>
            </div>
            <div class="file-actions">
                <button class="file-action-btn" onclick="recoveryPage.previewFile(${file.id})" title="Preview">
                    <i class="fas fa-eye"></i>
                </button>
                <button class="file-action-btn" onclick="recoveryPage.downloadSingle(${file.id})" title="Download">
                    <i class="fas fa-download"></i>
                </button>
            </div>
        `;

        return fileItem;
    }

    toggleFileSelection(fileId) {
        if (this.selectedFiles.has(fileId)) {
            this.selectedFiles.delete(fileId);
        } else {
            this.selectedFiles.add(fileId);
        }
        this.updateSelectedCount();
    }

    toggleSelectAll() {
        const currentPageFiles = this.filteredFiles.slice(
            (this.currentPage - 1) * this.filesPerPage,
            this.currentPage * this.filesPerPage
        );

        const allSelected = currentPageFiles.every(file => 
            this.selectedFiles.has(file.id)
        );

        if (allSelected) {
            currentPageFiles.forEach(file => {
                this.selectedFiles.delete(file.id);
            });
        } else {
            currentPageFiles.forEach(file => {
                this.selectedFiles.add(file.id);
            });
        }

        this.renderFiles();
    }

    updateSelectedCount() {
        const downloadBtn = document.getElementById('download-selected');
        if (downloadBtn) {
            downloadBtn.disabled = this.selectedFiles.size === 0;
            downloadBtn.innerHTML = `
                <i class="fas fa-download"></i>
                Download Selected (${this.selectedFiles.size})
            `;
        }
    }

    updatePagination() {
        const totalPages = Math.ceil(this.filteredFiles.length / this.filesPerPage);
        
        const prevBtn = document.getElementById('prev-page');
        const nextBtn = document.getElementById('next-page');
        const currentSpan = document.getElementById('page-current');
        const totalSpan = document.getElementById('page-total');

        if (prevBtn) prevBtn.disabled = this.currentPage === 1;
        if (nextBtn) nextBtn.disabled = this.currentPage === totalPages;
        if (currentSpan) currentSpan.textContent = this.currentPage;
        if (totalSpan) totalSpan.textContent = totalPages;
    }

    updateStats() {
        const recoveredCount = document.getElementById('recovered-count');
        const totalSize = document.getElementById('total-size');
        const avgConfidence = document.getElementById('avg-confidence');

        if (recoveredCount) {
            recoveredCount.textContent = this.allFiles.length;
        }

        if (totalSize) {
            const total = this.allFiles.reduce((sum, file) => sum + file.size, 0);
            totalSize.textContent = this.formatFileSize(total);
        }

        if (avgConfidence) {
            const avg = this.allFiles.reduce((sum, file) => sum + file.confidence, 0) / this.allFiles.length;
            avgConfidence.textContent = Math.round(avg) + '%';
        }
    }

    previewFile(fileId) {
        const file = this.allFiles.find(f => f.id === fileId);
        if (!file) return;

        const modal = document.getElementById('preview-modal');
        const title = document.getElementById('preview-title');
        const content = document.getElementById('preview-content');

        if (title) title.textContent = `Preview: ${file.name}`;
        
        if (content) {
            content.innerHTML = `
                <div class="preview-info">
                    <div class="preview-item">
                        <strong>File Name:</strong> ${file.name}
                    </div>
                    <div class="preview-item">
                        <strong>File Type:</strong> ${file.type.charAt(0).toUpperCase() + file.type.slice(1)}
                    </div>
                    <div class="preview-item">
                        <strong>File Size:</strong> ${this.formatFileSize(file.size)}
                    </div>
                    <div class="preview-item">
                        <strong>Confidence:</strong> ${file.confidence}%
                    </div>
                    <div class="preview-item">
                        <strong>Recovery Path:</strong> ${file.path}
                    </div>
                    <div class="preview-item">
                        <strong>Date Created:</strong> ${file.dateCreated}
                    </div>
                </div>
                <div class="preview-placeholder">
                    <i class="fas ${this.getFileIcon(file.type)}" style="font-size: 64px; color: #4fd1c7; margin-bottom: 16px;"></i>
                    <p>Full preview not available for this file type</p>
                    <p style="font-size: 12px; color: #718096;">Download the file to view its contents</p>
                </div>
            `;
        }

        if (modal) modal.style.display = 'flex';
    }

    downloadSelected() {
        if (this.selectedFiles.size === 0) return;

        const selectedFileObjects = Array.from(this.selectedFiles).map(id => 
            this.allFiles.find(f => f.id === id)
        );

        this.showDownloadProgress(selectedFileObjects);
    }

    downloadSingle(fileId) {
        const file = this.allFiles.find(f => f.id === fileId);
        if (!file) return;

        this.showDownloadProgress([file]);
    }

    showDownloadProgress(files) {
        const modal = document.getElementById('download-modal');
        const fileCount = document.getElementById('download-file-count');
        const downloadSize = document.getElementById('download-size');
        const progressFill = document.getElementById('download-progress-fill');
        const status = document.getElementById('download-status');
        const percentage = document.getElementById('download-percentage');

        const totalSize = files.reduce((sum, file) => sum + file.size, 0);

        if (fileCount) fileCount.textContent = files.length;
        if (downloadSize) downloadSize.textContent = this.formatFileSize(totalSize);
        if (modal) modal.style.display = 'flex';

        // Simulate download progress
        let progress = 0;
        const interval = setInterval(() => {
            progress += Math.random() * 15;
            if (progress >= 100) {
                progress = 100;
                clearInterval(interval);
                setTimeout(() => {
                    if (modal) modal.style.display = 'none';
                    this.showNotification('Download completed successfully!', 'success');
                }, 1000);
            }

            if (progressFill) progressFill.style.width = progress + '%';
            if (percentage) percentage.textContent = Math.round(progress) + '%';
            if (status) {
                if (progress < 100) {
                    status.textContent = `Downloading files... (${Math.round(progress)}%)`;
                } else {
                    status.textContent = 'Download complete!';
                }
            }
        }, 300);
    }

    toggleSection(targetId) {
        const content = document.getElementById(targetId);
        const btn = document.querySelector(`[data-target="${targetId}"]`);
        
        if (!content || !btn) return;

        const isCollapsed = content.style.display === 'none';
        
        content.style.display = isCollapsed ? 'block' : 'none';
        btn.innerHTML = isCollapsed ? '<i class="fas fa-chevron-up"></i>' : '<i class="fas fa-chevron-down"></i>';
    }

    exportReport() {
        this.showNotification('Generating export report...', 'info');
        
        // Simulate export process
        setTimeout(() => {
            this.showNotification('Recovery report exported successfully!', 'success');
        }, 2000);
    }

    saveSession() {
        this.showNotification('Saving recovery session...', 'info');
        
        // Simulate save process
        setTimeout(() => {
            this.showNotification('Recovery session saved successfully!', 'success');
        }, 1500);
    }

    getConfidenceClass(confidence) {
        if (confidence >= 80) return 'high';
        if (confidence >= 60) return 'medium';
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

    formatFileSize(bytes) {
        if (bytes === 0) return '0 B';
        
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
    }

    showNotification(message, type = 'info') {
        // Create notification element
        const notification = document.createElement('div');
        notification.className = `notification notification-${type}`;
        notification.style.cssText = `
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
        `;

        notification.innerHTML = `
            <div style="display: flex; align-items: center; gap: 12px; color: #e1e5e9;">
                <i class="fas ${this.getNotificationIcon(type)}"></i>
                <span>${message}</span>
                <button onclick="this.parentElement.parentElement.remove()" 
                        style="background: none; border: none; color: #a0aec0; cursor: pointer; padding: 4px; margin-left: auto;">
                    <i class="fas fa-times"></i>
                </button>
            </div>
        `;

        // Add border color based on type
        const borderColors = {
            'success': '#38a169',
            'error': '#e53e3e',
            'warning': '#d69e2e',
            'info': '#4fd1c7'
        };
        notification.style.borderLeft = `4px solid ${borderColors[type] || '#4fd1c7'}`;

        // Add to page
        document.body.appendChild(notification);

        // Animate in
        setTimeout(() => {
            notification.style.transform = 'translateX(0)';
        }, 100);

        // Auto-remove after 5 seconds
        setTimeout(() => {
            if (notification.parentElement) {
                notification.style.transform = 'translateX(100%)';
                setTimeout(() => {
                    notification.remove();
                }, 300);
            }
        }, 5000);
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
}

// Global functions for HTML onclick events
function closePreview() {
    const modal = document.getElementById('preview-modal');
    if (modal) modal.style.display = 'none';
}

function downloadSingleFile() {
    // This would be implemented to download the currently previewed file
    recoveryPage.showNotification('Download started...', 'success');
    closePreview();
}

function cancelDownload() {
    const modal = document.getElementById('download-modal');
    if (modal) modal.style.display = 'none';
    recoveryPage.showNotification('Download cancelled', 'info');
}

// Initialize the recovery page when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.recoveryPage = new RecoveryPage();
});