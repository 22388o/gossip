<script setup>
    import { reactive } from 'vue'
    import { useEventStore } from '../eventStore.js'
    import { invoke } from '@tauri-apps/api/tauri'

    const pagestate = reactive({
        redraw: 1,
        alert: null,
        password: "",
        password2: "",
        password3: "",
        password4: "",
        private_key: "",
    });

    const store = useEventStore()

    store.$subscribe((mutation, state) => {
        pagestate.redraw += 1;
    })

    function import_key() {
        pagestate.alert = null;

        if (pagestate.password != pagestate.password2) {
            pagestate.alert = "Passwords do not match.";
            return;
        }

        let password_copy = pagestate.password;
        pagestate.password = "00000000000000000000000";
        pagestate.password = "";
        pagestate.password2 = "00000000000000000000000";
        pagestate.password2 = "";

        // Unfortunately I don't know how to ensure the security once Tauri
        // get ahold of this password.
        invoke('import_key', { privatekey: pagestate.private_key, password: password_copy })
            .then((public_key) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
                store.public_key = public_key;
            })
            .catch((error) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
                pagestate.alert = error
            })
    }

    function generate() {
        pagestate.alert = null;

        if (pagestate.password3 != pagestate.password4) {
            pagestate.alert = "Passwords do not match.";
            return;
        }

        let password_copy = pagestate.password3;
        pagestate.password3 = "00000000000000000000000";
        pagestate.password3= "";
        pagestate.password4 = "00000000000000000000000";
        pagestate.password4 = "";

        // Unfortunately I don't know how to ensure the security once Tauri
        // get ahold of this password.
        invoke('generate', { password: password_copy })
            .then((success) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
            })
            .catch((error) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
                pagestate.alert = error
            })
    }

    function unlock() {
        pagestate.alert = null;

        let password_copy = pagestate.password;
        pagestate.password = "00000000000000000000000";
        pagestate.password = "";
        // Unfortunately I don't know how to ensure the security once Tauri
        // get ahold of this password.
        invoke('unlock', { password: password_copy })
            .then((success) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
            })
            .catch((error) => {
                password_copy = "00000000000000000000000";
                password_copy = "";
                pagestate.alert = error
            })
    }

    function key_security(ks) {
        if (ks==0) return "Weak (Imported/Exposed)";
        if (ks==1) return "Medium";
        return "Unknown";
    }
</script>

<template>
    <h2>yourself</h2>
    <div class="main-scrollable">
        <div v-if="pagestate.alert!=null" class="center alert">
            {{ pagestate.alert }}
        </div>

        <div v-if="store.public_key">
            Public Key: {{ store.public_key }}<br>
            <br>
            Key Security: {{ key_security(store.key_security) }}
        </div>
        <div v-else-if="store.need_password">
            Enter Password to Unlock Private Key:<br>
            Password: <input type="password" v-model="pagestate.password" @keyup.enter="unlock()" />
            <button @click="unlock()">Unlock</button>
        </div>
        <div v-else>
            <h3>Create Your Identity</h3>

            <hr>
            <div>
                <h4>Import your Private Key (Weak Security)</h4>
                Private Key: <input type="password" size="64" v-model="pagestate.private_key" /><br>
                New Password: <input type="password" v-model="pagestate.password" /><br>
                Repeat Password: <input type="password" v-model="pagestate.password2" /><br>
                <button @click="import_key()">Import</button>
                <ul>
                    <li>By using this, your private key is likely displayed on the screen</li>
                    <li>By using this, your private key probably remains in unallocated memory via the cut-n-paste buffer</li>
                </ul>
            </div>

            <hr>
            <div>
                <h4>Generate a Private Key (Medium Security)</h4>
                New Password: <input type="password" v-model="pagestate.password3" /><br>
                Repeat Password: <input type="password" v-model="pagestate.password4" /><br>
                <button @click="generate()">Generate</button>
                <ul>
                    <li>You will need to provide a PIN to unlock your private key each time you use it, and we will promptly forget your private key and PIN after each event is signed.</li>
                    <li>We will never display this private key on the screen.</li>
                    <li>We will never write this private key to disk in unencrypted form.</li>
                    <li>We will zero the memory that held your private key (and PIN) before freeing it</li>
                </ul>
            </div>

            <hr>
            <div>
                <h4>Generate a Private Key on a Hardware Token (Strong Security)</h4>
                TBD.
                <ul>
                    <li>You will need a compatible physical hardware token.</li>
                    <li>You will need system libraries and configuration allowing us to access that token.</li>
                </ul>
            </div>
        </div>
    </div>
</template>

<style scoped>
    div.alert {
        font-size: 2em;
        border: 1px solid black;
    }
</style>
