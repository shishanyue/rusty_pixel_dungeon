# Shattered Pixel Dungeon (Rust & Bevy Rewrite)

![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![Bevy](https://img.shields.io/badge/Bevy-202020?style=for-the-badge&logo=bevy&logoColor=white)
![License](https://img.shields.io/badge/License-GPL%20v3-blue.svg)

This project is a modern re-imagining and rewrite of the popular open-source roguelike **Shattered Pixel Dungeon**, written entirely in **Rust** using the **Bevy** game engine.

## 🌟 About

Shattered Pixel Dungeon is a traditional roguelike dungeon crawler with randomized levels, enemies, and items. This rewrite aims to modernize the codebase, improve performance, and provide a solid foundation for future modding capabilities using the power of Rust and the ECS (Entity Component System) architecture provided by Bevy.

**Note:** This project is currently under heavy development. It is not yet fully playable.

### Features

*   **Rust & Bevy:** Built for performance and safety using Rust's modern toolchain and Bevy's data-oriented design.
*   **Procedural Generation:** Algorithms to generate random, unique dungeon levels.
*   **Turn-Based Logic:** A robust scheduling system for handling game turns and entity actions.
*   **Pixel Art Style:** Faithful recreation of the original visual style.
*   **Cross-Platform:** Designed to run on Windows, macOS, and Linux (and potentially Web in the future).

## 🚀 Getting Started

### Prerequisites

*   **Rust:** You need to have Rust installed. If you don't have it, you can install it via [rustup](https://rustup.rs/).
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```
*   **Cargo:** Comes included with the Rust installation.

### Installation

1.  Clone the repository:
    ```bash
    git clone https://github.com/[your-username]/[your-repo-name].git
    cd [your-repo-name]
    ```

2.  Run the game:
    ```bash
    cargo run --release
    ```

3.  (Optional) To build a release executable:
    ```bash
    cargo build --release
    ```
    The executable will be located in `target/release/[your-binary-name]`.

## 🎮 How to Play

*   **Desktop Controls:**
    *   **WASD / Arrow Keys:** Move the hero.
    *   **Space / Enter:** Wait a turn.
    *   **Mouse:** Interact with UI elements.

*(Add more specific controls here as they are implemented)*

## 🛠️ Development

This project uses the Entity Component System (ECS) architecture. Here is a brief overview of the directory structure:

```text
.
├── assets/          # Textures, fonts, audio files
├── src/
│   ├── main.rs      # Entry point
│   ├── components/  # ECS Components (structs defining data)
│   ├── systems/     # ECS Systems (logic running on components)
│   ├── states/      # Game States (Menu, Playing, GameOver)
│   └── utils/       # Helper functions
└── Cargo.toml       # Dependencies
```

### Roadmap

- [ ] Core Game Loop
- [ ] Hero Movement & Collision
- [ ] Dungeon Generation Algorithms
- [ ] Inventory System
- [ ] Combat Mechanics
- [ ] UI / HUD Implementation
- [ ] Item System (Potions, Scrolls, Weapons)
- [ ] Enemy AI
- [ ] Saving & Loading

## 🤝 Contributing

Contributions are what make the open-source community such an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

1.  Fork the Project
2.  Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3.  Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4.  Push to the Branch (`git push origin feature/AmazingFeature`)
5.  Open a Pull Request

## 📝 License

This project is licensed under the GNU Affero General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

*Note: This project is a derivative work based on [Shattered Pixel Dungeon](https://github.com/00-Evan/shattered-pixel-dungeon) by Evan Debenham (originally based on Watabou's Pixel Dungeon).*

## 🙏 Acknowledgments

*   [Watabou](https://github.com/watabou) for the original Pixel Dungeon.
*   [00-Evan](https://github.com/00-Evan) for Shattered Pixel Dungeon.
*   The [Bevy Engine](https://bevyengine.org/) community for the excellent framework and documentation.
