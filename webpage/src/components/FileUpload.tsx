import { getAllSharedFiles, FileInfo } from '../api/llm';

export function SharedFilesPanel() {
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function fetchFiles() {
      setLoading(true);
      setError(null);
      try {
        const result = await getAllSharedFiles();
        setFiles(result);
      } catch (e: any) {
        setError(e.message || 'Failed to fetch files');
      }
      setLoading(false);
    }
    fetchFiles();
    const interval = setInterval(fetchFiles, 5000);
    return () => clearInterval(interval);
  }, []);

  if (loading) return <div className="p-4 text-center">Loading shared files...</div>;
  if (error) return <div className="p-4 text-red-500">{error}</div>;

  return (
    <div className="p-4 bg-dark rounded-lg shadow-lg mt-4">
      <h2 className="text-lg font-bold mb-2 text-accent">Shared Files</h2>
      {files.length === 0 ? (
        <div className="text-gray-400">No files shared yet.</div>
      ) : (
        <ul className="space-y-2">
          {files.map(file => (
            <li key={file.filename + file.upload_time} className="flex items-center gap-3 bg-neutral-900 rounded px-3 py-2">
              <span className="font-mono text-sm text-bright">{file.filename}</span>
              <span className="text-xs text-gray-400">{(file.file_size / 1024).toFixed(1)} KB</span>
              <span className="text-xs text-gray-500">by {file.uploader_ip}</span>
              <a
                href={`${API_BASE_URL}/api/files/${encodeURIComponent(file.filename)}`}
                download={file.filename}
                className="ml-auto px-2 py-1 bg-accent text-white rounded hover:bg-accent-dark text-xs flex items-center gap-1"
                title="Download file"
              >
                <Download className="w-4 h-4" /> Download
              </a>
              {file.file_type.startsWith('image/') && (
                <a
                  href={`${API_BASE_URL}/api/files/${encodeURIComponent(file.filename)}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="px-2 py-1 bg-bright text-black rounded hover:bg-accent text-xs"
                  title="Preview image"
                >
                  <Image className="w-4 h-4" /> Preview
                </a>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
import React, { useState, useRef } from 'react';
import { API_BASE_URL } from '../api/llm';
import { Upload, X, File, Image, FileText, Download } from 'lucide-react';

interface FileInfo {
  filename: string;
  file_type: string;
  file_size: number;
  uploader_ip: string;
  upload_time: string;
}

interface FileUploadProps {
  onFileUploaded: (fileInfo: FileInfo) => void;
}

export function FileUpload({ onFileUploaded }: FileUploadProps) {
  const [isDragOver, setIsDragOver] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState(0);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
    
    const files = Array.from(e.dataTransfer.files);
    if (files.length > 0) {
      uploadFile(files[0]);
    }
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (files && files.length > 0) {
      uploadFile(files[0]);
    }
  };

  const uploadFile = async (file: File) => {
    // Validate file size (50MB limit)
    if (file.size > 50 * 1024 * 1024) {
      alert('File too large. Maximum size is 50MB.');
      return;
    }

    // Validate file type
    const allowedTypes = [
      'image/jpeg', 'image/png', 'image/gif', 'image/webp',
      'text/plain', 'application/pdf', 'text/markdown'
    ];
    
    if (!allowedTypes.includes(file.type)) {
      alert('File type not supported. Allowed types: Images (JPEG, PNG, GIF, WebP), Text files, PDF, Markdown');
      return;
    }

    setUploading(true);
    setUploadProgress(0);

    const formData = new FormData();
    formData.append('file', file);

    setUploadError(null);
    try {
      const candidateUrls = [
        `${API_BASE_URL}/api/upload`,
        `${API_BASE_URL}/upload`,
        `${API_BASE_URL}/app/api/upload`,
      ];

      let response: Response | null = null;
      let triedUrl: string | null = null;
      for (const u of candidateUrls) {
        try {
          console.log('Attempting upload to', u);
          const r = await fetch(u, { method: 'POST', body: formData });
          response = r;
          triedUrl = u;
          if (r.status !== 404) break; // if 404, try next; otherwise stop and handle
        } catch (err) {
          console.warn('Network error when trying', u, err);
          // continue to next
        }
      }

      if (!response) {
        setUploadError('Upload failed: could not reach server (network error)');
        alert('Upload failed: could not reach server (network error)');
        return;
      }

      if (response.status === 404) {
        const text = await response.text().catch(() => '[non-plaintext response]');
        console.error('Upload failed (404) from', triedUrl, text);
        setUploadError(`Upload failed (404) from ${triedUrl}: ${text}`);
        alert(`Upload failed (404): ${text}`);
        return;
      }

      if (!response.ok) {
        const text = await response.text().catch(() => '[non-plaintext response]');
        console.error('Upload failed', response.status, text);
        setUploadError(`Upload failed (${response.status}): ${text}`);
        alert(`Upload failed (${response.status}): ${text}`);
        return;
      }

      let result: any;
      try {
        result = await response.json();
      } catch (err) {
        const text = await response.text().catch(() => '[non-plaintext response]');
        console.error('Upload JSON parse failed:', err, text);
        setUploadError('Upload failed: invalid server response');
        alert(`Upload failed: invalid server response`);
        return;
      }

      if (result.success) {
        onFileUploaded(result.file_info);
        setUploadProgress(100);
      } else {
        console.error('Upload error from server:', result);
        const msg = result.message || 'server error';
        setUploadError(`Upload failed: ${msg}`);
        alert(`Upload failed: ${msg}`);
      }
    } catch (error: any) {
      console.error('Upload error:', error);
      setUploadError('Upload failed. Please try again. - ' + (error?.message || String(error)));
      alert('Upload failed. Please try again. - ' + (error?.message || String(error)));
    } finally {
      setUploading(false);
      setUploadProgress(0);
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    }
  };

  const getFileIcon = (fileType: string) => {
    if (fileType.startsWith('image/')) {
      return <Image className="w-5 h-5" />;
    } else if (fileType === 'application/pdf') {
      return <FileText className="w-5 h-5" />;
    } else {
      return <File className="w-5 h-5" />;
    }
  };

  const formatFileSize = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  return (
    <div className="relative">
      <div
        className={`border-2 border-dashed rounded-lg p-6 text-center transition-colors cursor-pointer ${
          isDragOver
            ? 'border-blue-400 bg-blue-400/10'
            : 'border-gray-600 hover:border-gray-500'
        } ${uploading ? 'pointer-events-none opacity-50' : ''}`}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        onClick={() => fileInputRef.current?.click()}
      >
        <input
          ref={fileInputRef}
          type="file"
          className="hidden"
          onChange={handleFileSelect}
          accept="image/*,.txt,.pdf,.md"
        />
        
        {uploading ? (
          <div className="space-y-2">
            <div className="w-8 h-8 mx-auto">
              <div className="animate-spin rounded-full h-8 w-8 border-t-2 border-b-2 border-blue-500"></div>
            </div>
            <p className="text-sm text-gray-400">Uploading...</p>
            <div className="w-full bg-gray-700 rounded-full h-2">
              <div
                className="bg-blue-500 h-2 rounded-full transition-all duration-300"
                style={{ width: `${uploadProgress}%` }}
              ></div>
            </div>
          </div>
        ) : (
          <div className="space-y-2">
            <Upload className="w-8 h-8 mx-auto text-gray-400" />
            <div>
              <p className="text-sm text-gray-300">
                Drag and drop files here, or <span className="text-blue-400">click to browse</span>
              </p>
              <p className="text-xs text-gray-500 mt-1">
                Images, PDF, Text files (Max 50MB)
              </p>
            </div>
          </div>
        )}
      </div>
      {uploadError && (
        <div className="mt-3 text-sm text-red-400 bg-red-900/20 p-3 rounded">
          {uploadError}
        </div>
      )}
    </div>
  );
}

export function FilePreview({ fileInfo }: { fileInfo: FileInfo }) {
  const [showPreview, setShowPreview] = useState(false);

  const getFileIcon = (fileType: string) => {
    if (fileType.startsWith('image/')) {
      return <Image className="w-4 h-4" />;
    } else if (fileType === 'application/pdf') {
      return <FileText className="w-4 h-4" />;
    } else {
      return <File className="w-4 h-4" />;
    }
  };

  const formatFileSize = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const handleDownload = () => {
    window.open(`/api/files/${encodeURIComponent(fileInfo.filename)}`, '_blank');
  };

  return (
    <div className="bg-[#1a1a1a] border border-gray-700 rounded-lg p-3 flex items-center gap-3">
      <div className="text-blue-400">
        {getFileIcon(fileInfo.file_type)}
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-sm text-white truncate">{fileInfo.filename}</p>
        <p className="text-xs text-gray-400">
          {formatFileSize(fileInfo.file_size)} • {new Date(fileInfo.upload_time).toLocaleString()}
        </p>
      </div>
      <div className="flex items-center gap-2">
        {fileInfo.file_type.startsWith('image/') && (
          <button
            onClick={() => setShowPreview(!showPreview)}
            className="text-gray-400 hover:text-white transition-colors"
            title="Preview"
          >
            👁️
          </button>
        )}
        <button
          onClick={handleDownload}
          className="text-gray-400 hover:text-white transition-colors"
          title="Download"
        >
          <Download className="w-4 h-4" />
        </button>
      </div>
      
      {showPreview && fileInfo.file_type.startsWith('image/') && (
        <div className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50">
          <div className="relative max-w-4xl max-h-4xl">
            <img
              src={`/api/files/${encodeURIComponent(fileInfo.filename)}`}
              alt={fileInfo.filename}
              className="max-w-full max-h-full object-contain"
            />
            <button
              onClick={() => setShowPreview(false)}
              className="absolute top-4 right-4 text-white bg-black bg-opacity-50 rounded-full p-2 hover:bg-opacity-75"
            >
              <X className="w-6 h-6" />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
