'use strict';

// The Volumio side of the EVO Sabre's second display: the settings page,
// the display's service, and the word to the display that the player is
// going down. The program the service runs (bin/evosabre-panel) draws the
// glass and reads the remote; this file touches neither.
//
// It also switches click prevention: a daemon that holds the audio output
// open (keepalive/audio-keepalive-daemon), and the contribution that puts
// it into the player's ALSA chain.

var libQ = require('kew');
var fs = require('fs-extra');
var path = require('path');
var exec = require('child_process').exec;
var remote = require('./lib/remote');
var form = require('./lib/form');
var keepalive = require('./lib/keepalive');
var volume = require('./lib/volume');

var SERVICE = 'evosabre-panel.service';
var KEEPALIVE_SERVICE = 'evosabre-keepalive.service';
var ASOUND_DIR = path.join(__dirname, 'asound');
var ASOUND_FILE = '/etc/asound.conf';
var REMOTE_STATUS = '/run/evosabre-panel/remote.json';
var USERCONFIG = '/boot/userconfig.txt';
var SPI_DEVICE = '/dev/spidev0.0';
var RC_DEVICES = '/sys/class/rc';
var BOOT_MARK = '# >>> audiophonics_evo_sabre';
var id = 'audiophonics_evo_sabre: ';

module.exports = EvoSabre;

function EvoSabre(context) {
    this.context = context;
    this.commandRouter = context.coreCommand;
    this.logger = context.logger;
    this.configManager = context.configManager;
    this.reloadTimer = null;
    this.chainTimer = null;
}

EvoSabre.prototype.onVolumioStart = function () {
    var configFile = this.commandRouter.pluginManager.getConfigurationFile(this.context, 'config.json');
    this.config = new (require('v-conf'))();
    this.config.loadFile(configFile);
    return libQ.resolve();
};

EvoSabre.prototype.getConfigurationFiles = function () {
    return ['config.json'];
};

EvoSabre.prototype.onStart = function () {
    var self = this;
    // Strings are loaded here again so a fresh install needs no restart.
    self.commandRouter.loadI18nStrings();
    // The plugin starts whether or not the display does: a display that
    // cannot come up is said on the page, and the settings stay reachable.
    return self.applyService().then(function () {
        self.askRestartIfNeeded();
        return self.applyClickPrevention();
    });
};

EvoSabre.prototype.onStop = function () {
    var self = this;
    if (self.reloadTimer) { clearTimeout(self.reloadTimer); self.reloadTimer = null; }
    if (self.chainTimer) { clearTimeout(self.chainTimer); self.chainTimer = null; }
    return self.systemctl('disable --now')
        .fail(function () { /* said in the log */ })
        .then(function () { return self.dropClickPrevention(); });
};

EvoSabre.prototype.onRestart = function () { };
EvoSabre.prototype.onInstall = function () { };
EvoSabre.prototype.onUninstall = function () { };

// The player is going down: the display says so before the system stops it.
EvoSabre.prototype.onVolumioShutdown = function () {
    return this.farewell('SIGUSR1');
};

EvoSabre.prototype.onVolumioReboot = function () {
    return this.farewell('SIGUSR2');
};

EvoSabre.prototype.farewell = function (signal) {
    var defer = libQ.defer();
    this.signal(signal).fin(function () {
        // The display looks for the signal twice a second; give it that.
        setTimeout(function () { defer.resolve(); }, 700);
    });
    return defer.promise;
};

// -----------------------------------------------------------------------------
// The display's service.
// -----------------------------------------------------------------------------

EvoSabre.prototype.systemctl = function (verb, service) {
    var self = this;
    var defer = libQ.defer();
    exec('/usr/bin/sudo /bin/systemctl ' + verb + ' ' + (service || SERVICE), { uid: 1000, gid: 1000 }, function (error, stdout, stderr) {
        if (error) {
            self.logger.error(id + 'systemctl ' + verb + ': ' + String(stderr || error).trim());
            defer.reject(new Error(String(stderr || error.message || error).trim()));
        } else {
            defer.resolve();
        }
    });
    return defer.promise;
};

// The service follows the Display switch: on, it is started now and with
// the system; off, it is stopped and stays down.
EvoSabre.prototype.applyService = function () {
    var self = this;
    var on = form.isOn(self.config.get('oled_active'), true);
    return self.systemctl(on ? 'enable --now' : 'disable --now').fail(function (e) {
        if (on) {
            self.commandRouter.pushToastMessage('error', self.commandRouter.getI18nString('EVOSABRE.PLUGIN_NAME'),
                self.commandRouter.getI18nString('EVOSABRE.SERVICE_FAILED').replace('{{error}}', e && e.message ? e.message : e));
        }
    });
};

// The service's main process, or 0 when it is not running.
EvoSabre.prototype.mainPid = function () {
    var defer = libQ.defer();
    exec('/bin/systemctl show --property MainPID --value ' + SERVICE, function (error, stdout) {
        var pid = error ? 0 : parseInt(String(stdout).trim(), 10);
        defer.resolve(isNaN(pid) ? 0 : pid);
    });
    return defer.promise;
};

// A signal to the display, which runs as this user. Resolves either way:
// a display that is down has nothing to be told.
EvoSabre.prototype.signal = function (signal) {
    var self = this;
    return self.mainPid().then(function (pid) {
        if (pid > 0) {
            try {
                process.kill(pid, signal);
            } catch (e) {
                self.logger.error(id + signal + ' to the display: ' + e.message);
            }
        }
    });
};

// The display reads the settings again. The configuration store writes a
// moment after the set, so the word follows a moment later.
EvoSabre.prototype.reloadDisplay = function () {
    var self = this;
    if (self.reloadTimer) { clearTimeout(self.reloadTimer); }
    self.reloadTimer = setTimeout(function () {
        self.reloadTimer = null;
        self.signal('SIGHUP');
    }, 1500);
};

// The display and the receiver are switched on in the boot configuration,
// which the installer wrote; until the player has restarted on it, one or
// both are not there. Asked once per start, and only when that is the case.
EvoSabre.prototype.askRestartIfNeeded = function () {
    var self = this;
    var written = false;
    try { written = fs.readFileSync(USERCONFIG, 'utf8').indexOf(BOOT_MARK) !== -1; } catch (e) { /* no boot configuration here */ }
    if (!written || (fs.existsSync(SPI_DEVICE) && remote.receiverPresent(RC_DEVICES))) {
        return;
    }
    self.commandRouter.broadcastMessage('openModal', {
        title: self.commandRouter.getI18nString('EVOSABRE.RESTART_TITLE'),
        message: self.commandRouter.getI18nString('EVOSABRE.RESTART_MSG'),
        size: 'lg',
        buttons: [
            { name: self.commandRouter.getI18nString('COMMON.RESTART'), class: 'btn btn-info', emit: 'reboot', payload: '' },
            { name: self.commandRouter.getI18nString('COMMON.CONTINUE'), class: 'btn btn-default', emit: 'closeModals', payload: '' }
        ]
    });
};

// -----------------------------------------------------------------------------
// Click prevention: the audio output held open, so the DAC's clocks do not
// stop between tracks.
// -----------------------------------------------------------------------------

// Volumio's ALSA configuration, written again from every plugin's
// contribution. Where the file changed, Volumio has had ALSA restore its
// stored mixer state, so the volume and the mute the player had are asked
// for again. Resolves whatever becomes of it: the page says the rest.
EvoSabre.prototype.rebuildAlsa = function () {
    var self = this;
    var defer = libQ.defer();
    var done = false;
    var was = self.volumeNow();
    var finish = function (result) {
        if (done) { return; }
        done = true;
        if (result && result.changed) { self.restoreVolume(was); }
        defer.resolve();
    };
    var asked;
    try {
        asked = self.commandRouter.executeOnPlugin('audio_interface', 'alsa_controller', 'updateALSAConfigFile');
    } catch (e) {
        self.logger.error(id + 'updateALSAConfigFile: ' + e);
        finish();
        return defer.promise;
    }
    if (asked && typeof asked.then === 'function') {
        asked.then(finish).fail(function () { finish(); });
    } else {
        finish();
    }
    return defer.promise;
};

EvoSabre.prototype.volumeNow = function () {
    try {
        return volume.held(this.commandRouter.volumioGetState());
    } catch (e) {
        return null;
    }
};

// The volume first, which also unmutes; the mute a moment later, once
// Volumio has the volume to come back to.
EvoSabre.prototype.restoreVolume = function (was) {
    var self = this;
    volume.steps(was).forEach(function (step, i) {
        setTimeout(function () {
            try {
                self.commandRouter.volumiosetvolume(step);
            } catch (e) {
                self.logger.error(id + 'volume: ' + e);
            }
        }, i * 1000);
    });
};

// Out of the chain: the daemon stopped, the contribution gone, the
// configuration written without it.
EvoSabre.prototype.dropClickPrevention = function () {
    var self = this;
    var file = path.join(ASOUND_DIR, keepalive.CONTRIBUTION);
    return self.systemctl('stop', KEEPALIVE_SERVICE)
        .fail(function () { /* not running */ })
        .then(function () {
            if (!fs.existsSync(file)) {
                return libQ.resolve();
            }
            try { fs.unlinkSync(file); } catch (e) { self.logger.error(id + file + ': ' + e.message); }
            return self.rebuildAlsa();
        });
};

// The switch as it stands: on, the contribution is written, the
// configuration rebuilt around it and the daemon started; off, all three
// are undone. Where the Audio Keepalive plugin already holds the output
// open, nothing is put behind it.
EvoSabre.prototype.applyClickPrevention = function () {
    var self = this;
    var name = self.commandRouter.getI18nString('EVOSABRE.PLUGIN_NAME');
    if (!form.isOn(self.config.get('click_prevention'), false)) {
        return self.dropClickPrevention();
    }
    if (keepalive.standaloneOn()) {
        self.commandRouter.pushToastMessage('info', name, self.commandRouter.getI18nString('EVOSABRE.CLICK_STANDALONE'));
        return self.dropClickPrevention();
    }
    try {
        fs.ensureDirSync(ASOUND_DIR);
        fs.writeFileSync(path.join(ASOUND_DIR, keepalive.CONTRIBUTION), keepalive.contribution(__dirname), 'utf8');
    } catch (e) {
        self.logger.error(id + 'click prevention: ' + e.message);
        self.commandRouter.pushToastMessage('error', name,
            self.commandRouter.getI18nString('EVOSABRE.CLICK_FAILED').replace('{{error}}', e.message));
        return libQ.resolve();
    }
    return self.rebuildAlsa()
        .then(function () { return self.systemctl('restart', KEEPALIVE_SERVICE); })
        .then(function () { self.checkChain(); })
        .fail(function (e) {
            self.commandRouter.pushToastMessage('error', name,
                self.commandRouter.getI18nString('EVOSABRE.CLICK_FAILED').replace('{{error}}', e && e.message ? e.message : e));
        });
};

// Volumio asks for contributions only from the plugins it knew when it
// started: one installed since joins the chain after a restart. Looked at a
// moment after the rebuild, and asked for once.
EvoSabre.prototype.checkChain = function () {
    var self = this;
    if (self.chainTimer) { clearTimeout(self.chainTimer); }
    self.chainTimer = setTimeout(function () {
        self.chainTimer = null;
        var asound = '';
        try { asound = fs.readFileSync(ASOUND_FILE, 'utf8'); } catch (e) { /* not written yet */ }
        if (keepalive.inChain(asound)) {
            return;
        }
        self.commandRouter.broadcastMessage('openModal', {
            title: self.commandRouter.getI18nString('EVOSABRE.CLICK_RESTART_TITLE'),
            message: self.commandRouter.getI18nString('EVOSABRE.CLICK_RESTART_MSG'),
            size: 'lg',
            buttons: [
                { name: self.commandRouter.getI18nString('COMMON.RESTART'), class: 'btn btn-info', emit: 'reboot', payload: '' },
                { name: self.commandRouter.getI18nString('COMMON.CONTINUE'), class: 'btn btn-default', emit: 'closeModals', payload: '' }
            ]
        });
    }, 5000);
};

// -----------------------------------------------------------------------------
// The settings page.
// -----------------------------------------------------------------------------

EvoSabre.prototype.getUIConfig = function () {
    var self = this;
    var defer = libQ.defer();
    var lang = self.commandRouter.sharedVars.get('language_code');
    self.commandRouter.i18nJson(path.join(__dirname, 'i18n', 'strings_' + lang + '.json'),
        path.join(__dirname, 'i18n', 'strings_en.json'),
        path.join(__dirname, 'UIConfig.json'))
        .then(function (uiconf) {
            form.fill(uiconf, function (key) {
                return self.config.has(key) ? self.config.get(key) : undefined;
            });
            defer.resolve(uiconf);
        })
        .fail(function (e) {
            self.logger.error(id + 'UI config: ' + e);
            defer.reject(new Error());
        });
    return defer.promise;
};

EvoSabre.prototype.setUIConfig = function () { };
EvoSabre.prototype.getConf = function (varName) { return this.config.get(varName); };
EvoSabre.prototype.setConf = function (varName, varValue) { this.config.set(varName, varValue); };

// The values of one section into the store, or a word about the first that
// cannot be taken. Returns whether they were saved.
EvoSabre.prototype.save = function (data, keys) {
    var self = this;
    var name = self.commandRouter.getI18nString('EVOSABRE.PLUGIN_NAME');
    var taken = form.take(data, keys);
    if (taken.bad) {
        self.commandRouter.pushToastMessage('error', name, self.commandRouter.getI18nString('EVOSABRE.BAD_NUMBER')
            .replace('{{field}}', self.commandRouter.getI18nString('EVOSABRE.' + taken.bad.key.toUpperCase()))
            .replace('{{min}}', taken.bad.min)
            .replace('{{max}}', taken.bad.max));
        return false;
    }
    Object.keys(taken.values).forEach(function (key) {
        self.config.set(key, taken.values[key]);
    });
    self.commandRouter.pushToastMessage('success', name, self.commandRouter.getI18nString('EVOSABRE.SAVED'));
    return true;
};

EvoSabre.prototype.saveDisplay = function (data) {
    var self = this;
    var was = form.isOn(self.config.get('oled_active'), true);
    if (!self.save(data, ['oled_active', 'contrast', 'clock_after', 'sleep_after', 'deep_sleep_after'])) {
        return libQ.resolve();
    }
    if (form.isOn(self.config.get('oled_active'), true) !== was) {
        // A display just switched on starts before the store has written
        // what was saved with it; it is told to read again once it has.
        return self.applyService().then(function () {
            self.reloadDisplay();
        });
    }
    self.reloadDisplay();
    return libQ.resolve();
};

EvoSabre.prototype.saveRemote = function (data) {
    if (this.save(data, ['remote_active'])) {
        this.reloadDisplay();
    }
    return libQ.resolve();
};

EvoSabre.prototype.saveSound = function (data) {
    if (this.save(data, ['click_prevention'])) {
        return this.applyClickPrevention();
    }
    return libQ.resolve();
};

EvoSabre.prototype.saveJournal = function (data) {
    if (this.save(data, ['log_level'])) {
        this.reloadDisplay();
    }
    return libQ.resolve();
};

// What the running display says of its remote: whether the receiver is
// read, and the key that arrived last.
EvoSabre.prototype.checkRemote = function () {
    var self = this;
    var name = self.commandRouter.getI18nString('EVOSABRE.PLUGIN_NAME');
    var strings = {
        receiver: self.commandRouter.getI18nString('EVOSABRE.REMOTE_RECEIVER'),
        last: self.commandRouter.getI18nString('EVOSABRE.REMOTE_LAST'),
        none: self.commandRouter.getI18nString('EVOSABRE.REMOTE_NONE')
    };
    return self.mainPid().then(function (pid) {
        var status = null;
        if (pid > 0) {
            try { status = JSON.parse(fs.readFileSync(REMOTE_STATUS, 'utf8')); } catch (e) { /* not written yet */ }
        }
        if (!status) {
            self.commandRouter.pushToastMessage('warning', name, self.commandRouter.getI18nString('EVOSABRE.REMOTE_NOT_RUNNING'));
            return;
        }
        var report = remote.describe(status, Math.floor(Date.now() / 1000), strings);
        self.commandRouter.pushToastMessage(report.reading ? 'info' : 'warning', name, report.text);
    });
};

// called from commandrouter to find the language file
EvoSabre.prototype.getI18nFile = function (langCode) {
    var i18nFiles = fs.readdirSync(path.join(__dirname, 'i18n'));
    var langFile = 'strings_' + langCode + '.json';
    if (i18nFiles.some(function (f) { return f === langFile; })) {
        return path.join(__dirname, 'i18n', langFile);
    }
    return path.join(__dirname, 'i18n', 'strings_en.json');
};
