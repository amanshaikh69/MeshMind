export interface FileInfo {
  filename: string;
  file_type: string;
  file_size: number;
  uploader_ip: string;
  upload_time: string;
}

// -------- Auth --------
export type AuthStatus = { authenticated: boolean; username?: string };

export async function authStatus(): Promise<AuthStatus> {
  const res = await axios.get<AuthStatus>(`${API_ENDPOINT}/auth/status`);
  return res.data;
}

export async function login(username: string, password: string): Promise<AuthStatus> {
  const res = await axios.post<AuthStatus>(`${API_ENDPOINT}/auth/login`, { username, password });
  return res.data;
}

export async function logout(): Promise<void> {
  await axios.post(`${API_ENDPOINT}/auth/logout`);
}

export async function getAllSharedFiles(): Promise<FileInfo[]> {
  try {
    const response = await axios.get<FileInfo[]>(`${API_ENDPOINT}/files`);
    return response.data;
  } catch (error) {
    console.error('Error fetching shared files:', error);
    throw new Error('Failed to fetch shared files');
  }
}
import axios from 'axios';

// Use same-origin URLs so it works in production and dev (proxied)
export const API_BASE_URL = '';
export const API_ENDPOINT = `/api`;

export interface Message {
  role: 'user' | 'assistant';
  content: string;
}

export interface ChatRequest {
  message: string;
  sender: string;
}

export interface Conversation {
  id: string;
  messages: {
    content: string;
    timestamp: string;
    sender: string;
    message_type: 'Question' | 'Response';
    host_info: {
      hostname: string;
      ip_address: string;
      is_llm_host: boolean;
    };
  }[];
  host_info: {
    hostname: string;
    ip_address: string;
    is_llm_host: boolean;
  };
}

export async function sendMessageToLLM(message: string, filename?: string): Promise<string> {
  try {
    const payload: any = { message, sender: 'user' };
    if (filename) payload.filename = filename;
    const response = await axios.post<{ content: string }>(`${API_ENDPOINT}/chat`, payload);
    return response.data.content;
  } catch (error) {
    console.error('Error sending message to LLM:', error);
    throw new Error('Failed to get response from LLM');
  }
}

export async function getPeerConversations(): Promise<Record<string, Conversation>> {
  try {
    console.log('Fetching peer conversations from:', `/peers`);
    const response = await axios.get<Record<string, Conversation>>(`/peers`, {
      headers: {
        'Accept': 'application/json',
        'Content-Type': 'application/json'
      }
    });
    console.log('Response status:', response.status);
    console.log('Response headers:', response.headers);
    console.log('Received peer conversations:', response.data);
    
    if (!response.data || typeof response.data !== 'object') {
      console.error('Invalid response data:', response.data);
      throw new Error('Invalid response format');
    }
    
    return response.data;
  } catch (error) {
    if (axios.isAxiosError(error)) {
      console.error('Error fetching peer conversations:', {
        status: error.response?.status,
        statusText: error.response?.statusText,
        data: error.response?.data,
        headers: error.response?.headers,
        error: error.message,
        config: {
          url: error.config?.url,
          method: error.config?.method,
          headers: error.config?.headers
        }
      });
    } else {
      console.error('Error fetching peer conversations:', error);
    }
    throw new Error('Failed to get peer conversations');
  }
}

export async function getLocalConversation(): Promise<Conversation | null> {
  try {
    const response = await axios.get<Conversation | null>(`${API_BASE_URL}/api/local`);
    console.log('Received local conversation:', response.data);
    return response.data;
  } catch (error) {
    console.error('Error fetching local conversation:', error);
    return null;
  }
}