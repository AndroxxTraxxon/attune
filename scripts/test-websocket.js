#!/usr/bin/env node

/**
 * Test script to verify WebSocket event notifications from the Attune notifier service
 *
 * Usage: node scripts/test-websocket.js
 */

const WebSocket = require('ws');

const WS_URL = process.env.NOTIFIER_URL || 'ws://localhost:8081/ws';
const RECONNECT_DELAY = 3000;

console.log('🔌 Connecting to Attune Notifier Service...');
console.log(`   URL: ${WS_URL}\n`);

let ws;
let messageCount = 0;
let eventCount = 0;

function connect() {
  ws = new WebSocket(WS_URL);

  ws.on('open', () => {
    console.log('✅ Connected to notifier service');
    console.log('📡 Subscribing to event notifications...\n');

    // Subscribe to all event notifications
    ws.send(JSON.stringify({
      type: 'subscribe',
      filter: 'entity_type:event'
    }));
  });

  ws.on('message', (data) => {
    messageCount++;

    try {
      const message = JSON.parse(data.toString());

      if (message.type === 'welcome') {
        console.log('👋 Welcome message received');
        console.log(`   Client ID: ${message.client_id}`);
        console.log(`   Message: ${message.message}\n`);
      } else if (message.notification_type) {
        // This is a notification
        eventCount++;

        const timestamp = new Date(message.timestamp).toLocaleTimeString();
        console.log(`🔔 [${timestamp}] Event notification #${eventCount}`);
        console.log(`   Type: ${message.notification_type}`);
        console.log(`   Entity: ${message.entity_type} (ID: ${message.entity_id})`);

        if (message.payload && message.payload.data) {
          const data = message.payload.data;
          console.log(`   Trigger: ${data.trigger_ref || 'N/A'}`);
          console.log(`   Source: ${data.source_ref || 'N/A'}`);
        }
        console.log('');
      } else {
        console.log('📨 Unknown message format:', message);
      }
    } catch (error) {
      console.error('❌ Failed to parse message:', error.message);
      console.error('   Raw data:', data.toString());
    }
  });

  ws.on('error', (error) => {
    console.error('❌ WebSocket error:', error.message);
  });

  ws.on('close', () => {
    console.log('\n🔌 Connection closed');
    console.log(`   Total messages: ${messageCount}`);
    console.log(`   Event notifications: ${eventCount}`);
    console.log(`\n⏳ Reconnecting in ${RECONNECT_DELAY}ms...`);

    setTimeout(connect, RECONNECT_DELAY);
  });
}

// Handle graceful shutdown
process.on('SIGINT', () => {
  console.log('\n\n👋 Shutting down...');
  console.log(`   Total messages received: ${messageCount}`);
  console.log(`   Event notifications: ${eventCount}`);

  if (ws) {
    ws.close();
  }

  process.exit(0);
});

// Start connection
connect();

console.log('⏱️  Waiting for event notifications... (Press Ctrl+C to exit)\n');
