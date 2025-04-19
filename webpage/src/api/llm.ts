import axios from 'axios';

const API_BASE_URL = 'http://localhost:8080/api';

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
    const response = await axios.post<{ content: string }>(`${API_BASE_URL}/chat`, {
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
    const response = await axios.get<Record<string, Conversation>>(`${API_BASE_URL}/peers`);
    return response.data;
  } catch (error) {
    console.error('Error fetching peer conversations:', error);
    throw new Error('Failed to get peer conversations');
  }
}