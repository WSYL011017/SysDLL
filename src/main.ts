import { createApp } from 'vue'
import { createPinia } from 'pinia'
import 'virtual:uno.css'
import '@unocss/reset/tailwind.css'
import './styles/main.css'
import App from './App.vue'

const app = createApp(App)
app.use(createPinia())
app.mount('#app')
