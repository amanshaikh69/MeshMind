import axios from 'axios';

const API_BASE_URL = 'http://localhost:8080';
const API_ENDPOINT = `${API_BASE_URL}/api`;

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

export async function sendMessageToLLM(message: string): Promise<string> {
  try {
    const response = await axios.post<{ content: string }>(`${API_ENDPOINT}/chat`, {
      message,
      sender: 'user',
    });
    return response.data.content;
  } catch (error) {
    console.error('Error sending message to LLM:', error);
    throw new Error('Failed to get response from LLM');
  }
}

export async function getPeerConversations(): Promise<Record<string, Conversation>> {
  try {
    console.log('Fetching peer conversations from:', `${API_BASE_URL}/peers`);
    const response = await axios.get<Record<string, Conversation>>(`${API_BASE_URL}/peers`, {
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