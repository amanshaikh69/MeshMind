import axios from 'axios';

const API_BASE_URL = '/api';

export interface Message {
  role: 'user' | 'assistant';
  content: string;
}

export interface ChatRequest {
  message: string;
  sender: string;
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