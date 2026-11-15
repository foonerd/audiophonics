'use strict';

// The Volumio side of the RaspDac Mini's display: the settings page, the
// display's service, the word to the display that the player is going
// down, and the one thing the display has to ask the player for through
// its plugin, a shutdown. The program the service runs
// (bin/raspdacmini-panel) draws the glass, reads the remote and sets the
// DAC; this file touches none of them.

var libQ = require('kew');
var fs = require('fs-extra');
var path = require('path');
var exec = require('child_process').exec;
var remote = require('./lib/remote');
var form = require('./lib/form');

var SERVICE = 'raspdacmini-panel.service';
var REMOTE_STATUS = '/run/raspdacmini-panel/remote.json';
var USERCONFIG = '/boot/userconfig.txt';
var SPI_DEVICE = '/dev/spidev0.0';
var GRAPHICS = '/sys/class/graphics';
var PANEL_FB = 'fb_ili9341';
var RC_DEVICES = '/sys/class/rc';
var BOOT_HELPER = '/usr/local/bin/raspdacmini-boot';
var BOOT_MARK = '# >>> raspdac_mini_lcd';
// The name the display's program calls this plugin by at the player.
var ENDPOINT = { endpoint: 'raspdac_mini_lcd', type: 'system_hardware', name: 'raspdac_mini_lcd', method: 'panelAsks' };
var id = 'raspdac_mini_lcd: ';

module.exports = RaspDacMini;

function RaspDacMini(context) {
    this.context = context;
    this.commandRouter = context.coreCommand;
    this.logger = context.logger;
    this.configManager = context.configManager;
    this.reloadTimer = null;
}

RaspDacMini.prototype.onVolumioStart = function () {
    var configFile = this.commandRouter.pluginManager.getConfigurationFile(this.context, 'config.json');
    this.config = new (require('v-conf'))();
    this.config.loadFile(configFile);
    return libQ.resolve();
};

RaspDacMini.prototype.getConfigurationFiles = function () {
    return ['config.json'];
};

RaspDacMini.prototype.onStart = function () {
    var self = this;
    // Strings are loaded here again so a fresh install needs no restart.
    self.commandRouter.loadI18nStrings();
    self.commandRouter.addPluginRestEndpoint(ENDPOINT);
    // The plugin starts whether or not the display does: a display that
    // cannot come up is said on the page, and the settings stay reachable.
    return self.applyService().then(function () {
        self.askRestartIfNeeded();
    });
};

RaspDacMini.prototype.onStop = function () {
    var self = this;
    if (self.reloadTimer) { clearTimeout(self.reloadTimer); self.reloadTimer = null; }
    self.commandRouter.removePluginRestEndpoint(ENDPOINT);
    return self.systemctl('disable --now').fail(function () { /* said in the log */ });
};

RaspDacMini.prototype.onRestart = function () { };
RaspDacMini.prototype.onInstall = function () { };
RaspDacMini.prototype.onUninstall = function () { };

// The player is going down: the display says so before the system stops it.
RaspDacMini.prototype.onVolumioShutdown = function () {
    return this.farewell('SIGUSR1');
};

RaspDacMini.prototype.onVolumioReboot = function () {
    return this.farewell('SIGUSR2');
};

RaspDacMini.prototype.farewell = function (signal) {
    var defer = libQ.defer();
    this.signal(signal).fin(function () {
        // The display looks for the signal twice a second; give it that.
        setTimeout(function () { defer.resolve(); }, 700);
    });
    return defer.promise;
};

// What the display's program asks of the player through this plugin. The
// remote's Power key is a shutdown, and only the player can shut the player
// down properly: it tells every plugin first, this one included.
RaspDacMini.prototype.panelAsks = function (word) {
    if (word === 'shutdown') {
        this.logger.info(id + 'the remote asked for a shutdown');
        this.commandRouter.shutdown();
        return libQ.resolve({ asked: 'shutdown' });
    }
    this.logger.error(id + 'asked for something it does not do: ' + String(word).slice(0, 40));
    return libQ.reject(new Error('not something this plugin does'));
};

// -----------------------------------------------------------------------------
// The display's service.
// -----------------------------------------------------------------------------

RaspDacMini.prototype.systemctl = function (verb) {
    var self = this;
    var defer = libQ.defer();
    exec('/usr/bin/sudo /bin/systemctl ' + verb + ' ' + SERVICE, { uid: 1000, gid: 1000 }, function (error, stdout, stderr) {
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
RaspDacMini.prototype.applyService = function () {
    var self = this;
    var on = form.isOn(self.config.get('lcd_active'), true);
    return self.systemctl(on ? 'enable --now' : 'disable --now').fail(function (e) {
        if (on) {
            self.commandRouter.pushToastMessage('error', self.commandRouter.getI18nString('RASPDACMINI.PLUGIN_NAME'),
                self.commandRouter.getI18nString('RASPDACMINI.SERVICE_FAILED').replace('{{error}}', e && e.message ? e.message : e));
        }
    });
};

// The service's main process, or 0 when it is not running.
RaspDacMini.prototype.mainPid = function () {
    var defer = libQ.defer();
    exec('/bin/systemctl show --property MainPID --value ' + SERVICE, function (error, stdout) {
        var pid = error ? 0 : parseInt(String(stdout).trim(), 10);
        defer.resolve(isNaN(pid) ? 0 : pid);
    });
    return defer.promise;
};

// A signal to the display, which runs as this user. Resolves either way:
// a display that is down has nothing to be told.
RaspDacMini.prototype.signal = function (signal) {
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
RaspDacMini.prototype.reloadDisplay = function () {
    var self = this;
    if (self.reloadTimer) { clearTimeout(self.reloadTimer); }
    self.reloadTimer = setTimeout(function () {
        self.reloadTimer = null;
        self.signal('SIGHUP');
    }, 1500);
};

// Whether the panel is there the way the settings have it driven: as the
// kernel's framebuffer for it, or as the SPI device.
RaspDacMini.prototype.panelPresent = function () {
    if (form.driver(this.config.get('display_driver')) === 'spi') {
        return fs.existsSync(SPI_DEVICE);
    }
    var names;
    try { names = fs.readdirSync(GRAPHICS); } catch (e) { return false; }
    return names.some(function (name) {
        try {
            return fs.readFileSync(path.join(GRAPHICS, name, 'name'), 'utf8').trim() === PANEL_FB;
        } catch (e) {
            return false;
        }
    });
};

// The plugin's lines in the boot configuration, as the Display driver
// setting has them. Resolves with whether the file changed, which is when
// the player has to restart; resolves false, with a word on the page, when
// it could not be written.
RaspDacMini.prototype.applyBoot = function () {
    var self = this;
    var defer = libQ.defer();
    var driver = form.driver(self.config.get('display_driver'));
    exec('/usr/bin/sudo ' + BOOT_HELPER + ' ' + driver, { uid: 1000, gid: 1000 }, function (error, stdout, stderr) {
        if (error) {
            var why = String(stderr || error.message || error).trim();
            self.logger.error(id + 'boot configuration: ' + why);
            self.commandRouter.pushToastMessage('error', self.commandRouter.getI18nString('RASPDACMINI.PLUGIN_NAME'),
                self.commandRouter.getI18nString('RASPDACMINI.BOOT_FAILED').replace('{{error}}', why));
            defer.resolve(false);
        } else {
            defer.resolve(String(stdout).trim() === 'changed');
        }
    });
    return defer.promise;
};

RaspDacMini.prototype.askRestart = function (message) {
    var self = this;
    self.commandRouter.broadcastMessage('openModal', {
        title: self.commandRouter.getI18nString('RASPDACMINI.RESTART_TITLE'),
        message: self.commandRouter.getI18nString(message),
        size: 'lg',
        buttons: [
            { name: self.commandRouter.getI18nString('COMMON.RESTART'), class: 'btn btn-info', emit: 'reboot', payload: '' },
            { name: self.commandRouter.getI18nString('COMMON.CONTINUE'), class: 'btn btn-default', emit: 'closeModals', payload: '' }
        ]
    });
};

// The display and the receiver are switched on in the boot configuration,
// which the installer wrote; until the player has restarted on it, one or
// both are not there. Asked once per start, and only when that is the case.
RaspDacMini.prototype.askRestartIfNeeded = function () {
    var self = this;
    var written = false;
    try { written = fs.readFileSync(USERCONFIG, 'utf8').indexOf(BOOT_MARK) !== -1; } catch (e) { /* no boot configuration here */ }
    if (!written || (self.panelPresent() && remote.receiverPresent(RC_DEVICES))) {
        return;
    }
    self.commandRouter.broadcastMessage('openModal', {
        title: self.commandRouter.getI18nString('RASPDACMINI.RESTART_TITLE'),
        message: self.commandRouter.getI18nString('RASPDACMINI.RESTART_MSG'),
        size: 'lg',
        buttons: [
            { name: self.commandRouter.getI18nString('COMMON.RESTART'), class: 'btn btn-info', emit: 'reboot', payload: '' },
            { name: self.commandRouter.getI18nString('COMMON.CONTINUE'), class: 'btn btn-default', emit: 'closeModals', payload: '' }
        ]
    });
};

// -----------------------------------------------------------------------------
// The settings page.
// -----------------------------------------------------------------------------

RaspDacMini.prototype.getUIConfig = function () {
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

RaspDacMini.prototype.setUIConfig = function () { };
RaspDacMini.prototype.getConf = function (varName) { return this.config.get(varName); };
RaspDacMini.prototype.setConf = function (varName, varValue) { this.config.set(varName, varValue); };

// The values of one section into the store, or a word about the first that
// cannot be taken. Returns whether they were saved.
RaspDacMini.prototype.save = function (data, keys) {
    var self = this;
    var name = self.commandRouter.getI18nString('RASPDACMINI.PLUGIN_NAME');
    var taken = form.take(data, keys);
    if (taken.bad) {
        self.commandRouter.pushToastMessage('error', name, self.commandRouter.getI18nString('RASPDACMINI.BAD_NUMBER')
            .replace('{{field}}', self.commandRouter.getI18nString('RASPDACMINI.' + taken.bad.key.toUpperCase()))
            .replace('{{min}}', taken.bad.min)
            .replace('{{max}}', taken.bad.max));
        return false;
    }
    Object.keys(taken.values).forEach(function (key) {
        self.config.set(key, taken.values[key]);
    });
    self.commandRouter.pushToastMessage('success', name, self.commandRouter.getI18nString('RASPDACMINI.SAVED'));
    return true;
};

RaspDacMini.prototype.saveDisplay = function (data) {
    var self = this;
    var was = form.isOn(self.config.get('lcd_active'), true);
    var driven = form.driver(self.config.get('display_driver'));
    if (!self.save(data, ['lcd_active', 'display_driver', 'boot_splash', 'sleep_after'])) {
        return libQ.resolve();
    }
    if (form.driver(self.config.get('display_driver')) !== driven) {
        // The way the display is driven is in the boot configuration: the
        // display goes on as it is until the player has restarted on it.
        self.applyBoot().then(function (changed) {
            if (changed) {
                self.askRestart('RASPDACMINI.DRIVER_RESTART_MSG');
            }
        });
    }
    if (form.isOn(self.config.get('lcd_active'), true) !== was) {
        // A display just switched on starts before the store has written
        // what was saved with it; it is told to read again once it has.
        return self.applyService().then(function () {
            self.reloadDisplay();
        });
    }
    self.reloadDisplay();
    return libQ.resolve();
};

RaspDacMini.prototype.saveRemote = function (data) {
    if (this.save(data, ['remote_active'])) {
        this.reloadDisplay();
    }
    return libQ.resolve();
};

RaspDacMini.prototype.saveJournal = function (data) {
    if (this.save(data, ['log_level'])) {
        this.reloadDisplay();
    }
    return libQ.resolve();
};

// What the running display says of its remote: whether the receiver is
// read, and the key that arrived last.
RaspDacMini.prototype.checkRemote = function () {
    var self = this;
    var name = self.commandRouter.getI18nString('RASPDACMINI.PLUGIN_NAME');
    var strings = {
        receiver: self.commandRouter.getI18nString('RASPDACMINI.REMOTE_RECEIVER'),
        last: self.commandRouter.getI18nString('RASPDACMINI.REMOTE_LAST'),
        none: self.commandRouter.getI18nString('RASPDACMINI.REMOTE_NONE')
    };
    return self.mainPid().then(function (pid) {
        var status = null;
        if (pid > 0) {
            try { status = JSON.parse(fs.readFileSync(REMOTE_STATUS, 'utf8')); } catch (e) { /* not written yet */ }
        }
        if (!status) {
            self.commandRouter.pushToastMessage('warning', name, self.commandRouter.getI18nString('RASPDACMINI.REMOTE_NOT_RUNNING'));
            return;
        }
        var report = remote.describe(status, Math.floor(Date.now() / 1000), strings);
        self.commandRouter.pushToastMessage(report.reading ? 'info' : 'warning', name, report.text);
    });
};

// called from commandrouter to find the language file
RaspDacMini.prototype.getI18nFile = function (langCode) {
    var i18nFiles = fs.readdirSync(path.join(__dirname, 'i18n'));
    var langFile = 'strings_' + langCode + '.json';
    if (i18nFiles.some(function (f) { return f === langFile; })) {
        return path.join(__dirname, 'i18n', langFile);
    }
    return path.join(__dirname, 'i18n', 'strings_en.json');
};
