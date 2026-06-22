package main

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"syscall"
)

type ServerConfig struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Address     string `json:"address"`
	Port        int    `json:"port"`
	Protocol    string `json:"protocol"`
	UUID        string `json:"uuid"`
	Flow        string `json:"flow"`
	SNI         string `json:"sni"`
	PublicKey   string `json:"public_key"`
	ShortID     string `json:"short_id"`
	Security    string `json:"security"`
	Fingerprint string `json:"fingerprint"`
	SpiderX     string `json:"spider_x"`
	Type        string `json:"type"`
}

type RoutingConfig struct {
	BlockAds     bool   `json:"block_ads"`
	BypassLan    bool   `json:"bypass_lan"`
	SplitRouting bool   `json:"split_routing"`
	Region       string `json:"region"`
}

var xrayProcess *exec.Cmd

func main() {
	if len(os.Args) < 2 {
		fmt.Println("Usage: max-strike-core <command> [args]")
		os.Exit(1)
	}

	command := os.Args[1]

	switch command {
	case "connect":
		if len(os.Args) < 3 {
			fmt.Println("Usage: max-strike-core connect <config.json>")
			os.Exit(1)
		}
		connect(os.Args[2])
	case "disconnect":
		disconnect()
	default:
		fmt.Printf("Unknown command: %s\n", command)
		os.Exit(1)
	}
}

func connect(configPath string) {
	data, err := os.ReadFile(configPath)
	if err != nil {
		fmt.Printf("Failed to read config: %v\n", err)
		os.Exit(1)
	}

	var config ServerConfig
	if err := json.Unmarshal(data, &config); err != nil {
		fmt.Printf("Failed to parse config: %v\n", err)
		os.Exit(1)
	}

	if config.Fingerprint == "" {
		config.Fingerprint = "firefox"
	}
	if config.SpiderX == "" {
		config.SpiderX = "/"
	}
	if config.Type == "" {
		config.Type = "tcp"
	}

	routing := RoutingConfig{
		BlockAds:     false,
		BypassLan:    true,
		SplitRouting: true,
		Region:       "russia",
	}
	
	routingPath := os.Getenv("ROUTING_CONFIG")
	if routingPath != "" {
		if routingData, err := os.ReadFile(routingPath); err == nil {
			json.Unmarshal(routingData, &routing)
			fmt.Printf("Routing loaded: block_ads=%v, bypass_lan=%v, split_routing=%v, region=%s\n",
				routing.BlockAds, routing.BypassLan, routing.SplitRouting, routing.Region)
		}
	}

	xrayConfig := createXrayConfig(config, routing)

	tmpDir := os.TempDir()
	xrayConfigPath := filepath.Join(tmpDir, "max-strike-xray-config.json")

	configJSON, _ := json.MarshalIndent(xrayConfig, "", "  ")
	if err := os.WriteFile(xrayConfigPath, configJSON, 0644); err != nil {
		fmt.Printf("Failed to write xray config: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Xray config saved to: %s\n", xrayConfigPath)

	xrayPath := findXray()
	if xrayPath == "" {
		fmt.Println("Xray binary not found")
		os.Exit(1)
	}

	fmt.Printf("Using xray: %s\n", xrayPath)

	xrayProcess = exec.Command(xrayPath, "run", "-config", xrayConfigPath)
	xrayProcess.Stdout = os.Stdout
	xrayProcess.Stderr = os.Stderr

	if err := xrayProcess.Start(); err != nil {
		fmt.Printf("Failed to start xray: %v\n", err)
		os.Exit(1)
	}

	fmt.Println("Connected successfully")
	fmt.Println("SOCKS5 proxy: 127.0.0.1:10808")

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
	<-sigChan

	disconnect()
}

func createXrayConfig(config ServerConfig, routing RoutingConfig) map[string]interface{} {
	outbounds := []map[string]interface{}{
		createOutbound(config),
	}

	outbounds = append(outbounds, map[string]interface{}{
		"tag":      "direct",
		"protocol": "freedom",
		"settings": map[string]interface{}{},
	})

	outbounds = append(outbounds, map[string]interface{}{
		"tag":      "block",
		"protocol": "blackhole",
		"settings": map[string]interface{}{},
	})

	dnsConfig := createDNSConfig(routing)
	routingRules := createRoutingRules(routing)

	xrayConfig := map[string]interface{}{
		"log": map[string]interface{}{
			"loglevel": "warning",
		},
		"dns": dnsConfig,
		"inbounds": []map[string]interface{}{
			{
				"port":     10808,
				"listen":   "127.0.0.1",
				"protocol": "socks",
				"sniffing": map[string]interface{}{
					"enabled":      true,
					"destOverride": []string{"http", "tls"},
				},
				"settings": map[string]interface{}{
					"auth":      "noauth",
					"udp":       true,
					"userLevel": 0,
				},
			},
			{
				"port":     10809,
				"listen":   "127.0.0.1",
				"protocol": "http",
				"sniffing": map[string]interface{}{
					"enabled":      true,
					"destOverride": []string{"http", "tls"},
				},
				"settings": map[string]interface{}{
					"timeout": 0,
				},
			},
		},
		"outbounds": outbounds,
		"routing":   routingRules,
	}

	return xrayConfig
}

func createDNSConfig(routing RoutingConfig) map[string]interface{} {
	servers := []interface{}{}

	if routing.BlockAds {
		servers = append(servers, map[string]interface{}{
			"address": "https://dns.adguard-dns.com/dns-query",
			"domains": []string{"geosite:category-ads-all"},
		})
	}

	if routing.SplitRouting && routing.Region == "russia" {
		servers = append(servers, map[string]interface{}{
			"address": "https://common.dot.dns.yandex.net/dns-query",
			"domains": []string{"geosite:category-ru"},
		})
	}

	servers = append(servers, "https://8.8.8.8/dns-query")

	return map[string]interface{}{
		"servers": servers,
	}
}

func createRoutingRules(routing RoutingConfig) map[string]interface{} {
	rules := []map[string]interface{}{}

	// 1. Блокировка рекламы
	if routing.BlockAds {
		rules = append(rules, map[string]interface{}{
			"type":        "field",
			"outboundTag": "block",
			"domain":      []string{"geosite:category-ads-all"},
		})
	}

	// 2. Обход LAN
	if routing.BypassLan {
		rules = append(rules, map[string]interface{}{
			"type":        "field",
			"outboundTag": "direct",
			"ip": []string{
				"geoip:private",
			},
		})
	}

	// 3. Разделение трафика - РФ напрямую
	if routing.SplitRouting && routing.Region == "russia" {
		// Российские домены напрямую (используем geosite от Loyalsoldier)
		rules = append(rules, map[string]interface{}{
			"type":        "field",
			"outboundTag": "direct",
			"domain":      []string{"geosite:category-ru"},
		})
		
		// Российские IP напрямую (используем geoip от v2fly)
		rules = append(rules, map[string]interface{}{
			"type":        "field",
			"outboundTag": "direct",
			"ip":          []string{"geoip:ru"},
		})
	}

	// 4. Всё остальное через прокси
	rules = append(rules, map[string]interface{}{
		"type":        "field",
		"outboundTag": "proxy",
		"network":     "tcp,udp",
	})

	return map[string]interface{}{
		"domainStrategy": "IPIfNonMatch",
		"rules":          rules,
	}
}

func createOutbound(config ServerConfig) map[string]interface{} {
	user := map[string]interface{}{
		"id":         config.UUID,
		"encryption": "none",
	}

	if config.Flow != "" {
		user["flow"] = config.Flow
	}

	outbound := map[string]interface{}{
		"tag":      "proxy",
		"protocol": config.Protocol,
		"settings": map[string]interface{}{
			"vnext": []map[string]interface{}{
				{
					"address": config.Address,
					"port":    config.Port,
					"users":   []map[string]interface{}{user},
				},
			},
		},
		"streamSettings": createStreamSettings(config),
	}

	return outbound
}

func createStreamSettings(config ServerConfig) map[string]interface{} {
	settings := map[string]interface{}{
		"network":  config.Type,
		"security": config.Security,
	}

	if config.Security == "reality" {
		settings["realitySettings"] = map[string]interface{}{
			"serverName":  config.SNI,
			"publicKey":   config.PublicKey,
			"shortId":     config.ShortID,
			"spiderX":     config.SpiderX,
			"fingerprint": config.Fingerprint,
		}
	} else if config.Security == "tls" {
		settings["tlsSettings"] = map[string]interface{}{
			"serverName": config.SNI,
		}
	}

	return settings
}

func findXray() string {
	if envPath := os.Getenv("XRAY_PATH"); envPath != "" {
		if _, err := os.Stat(envPath); err == nil {
			return envPath
		}
	}

	if exePath, err := os.Executable(); err == nil {
		exeDir := filepath.Dir(exePath)
		localXray := filepath.Join(exeDir, "xray")
		if _, err := os.Stat(localXray); err == nil {
			return localXray
		}
	}

	if path, err := exec.LookPath("xray"); err == nil {
		return path
	}

	paths := []string{
		"/usr/local/bin/xray",
		"/usr/bin/xray",
		"/opt/xray/xray",
		"./xray",
	}

	for _, p := range paths {
		if _, err := os.Stat(p); err == nil {
			return p
		}
	}

	return ""
}

func disconnect() {
	if xrayProcess != nil && xrayProcess.Process != nil {
		xrayProcess.Process.Kill()
		xrayProcess.Wait()
		xrayProcess = nil
		fmt.Println("Disconnected")
	}
}
