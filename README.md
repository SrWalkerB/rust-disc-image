# rust-disc-image

Bibliotecas Rust para ler imagens de disco óptico: acesso a setores, sistema de arquivos ISO 9660 e metadados de discos de PlayStation 2.

Status: primeira versão dos três crates, testada com imagens sintéticas e validada em leitura de uma imagem real de DVD de dupla camada. Ainda não há fuzzing.

```powershell
cargo test --workspace
cargo run -p ps2-disc --example ps2-info -- "caminho\para\imagem.iso"
```

## Crates

O repositório será um workspace Cargo com três crates de biblioteca, em camadas. Cada camada depende apenas das anteriores.

| Crate | Responsabilidade | Depende de |
| --- | --- | --- |
| `disc-image` | Ler setores por LBA a partir de uma imagem de disco. | — |
| `iso9660` | Interpretar o sistema de arquivos ISO 9660: descritores de volume, diretórios, arquivos e a posição (LBA e tamanho) de cada um. | `disc-image` |
| `ps2-disc` | Identificar discos de PS2: `SYSTEM.CNF`, serial, ELF de boot, inventário de módulos IRX e cabeçalho do ELF. | `disc-image`, `iso9660` |

Os crates `disc-image` e `iso9660` não conhecem nenhum console. Tudo que for específico do PS2 fica em `ps2-disc`, e tudo que for específico de um jogo fica no projeto que usa estas bibliotecas.

Layout:

```text
Cargo.toml          workspace
crates/
  disc-image/       IsoFile (arquivo .iso) e MemoryImage, sobre o trait SectorSource
  iso9660/          Volume: read_dir, lookup, walk, read_at, read_to_vec
  ps2-disc/         Ps2Disc: camadas, SYSTEM.CNF, ELF de boot, arquivos e IRX
  iso-test-builder/ monta imagens sintéticas para os testes (não publicado)
```

## Dupla camada

Verificado em uma imagem real de DVD9 de PS2: a camada 1 é um segundo volume ISO 9660 cujo setor 0 fica 16 setores antes do fim do volume da camada 0, de modo que seu descritor primário aparece logo depois da camada 0. Os LBAs gravados na camada 1 são relativos a esse início. `Volume::open_at` abre um volume a partir de qualquer LBA base, e `Ps2Disc::open` detecta a segunda camada dessa forma. Todas as entradas expõem LBAs absolutos na imagem.

## Escopo inicial

- **Formatos de imagem:** a interface de leitura de setores será preparada para vários formatos, mas só `.iso` com setores de 2048 bytes será implementado agora. BIN/CUE (setores de 2352 bytes), CSO e CHD ficam para depois.
- **Sistema de arquivos:** ISO 9660. Joliet e UDF ficam fora do escopo inicial; para discos de PS2, ISO 9660 normalmente basta.
- **PS2:** leitura de `SYSTEM.CNF` (`BOOT2`, `VER`, `VMODE`), serial, localização do ELF de boot, inventário de IRX e leitura do cabeçalho ELF.
- **Fora do escopo:** criar ou modificar imagens, montar o disco no sistema operacional e interpretar formatos de arquivo internos de jogos.

## Requisitos de projeto

- **Somente leitura e por streaming.** Imagens de DVD chegam a 8,5 GB; a leitura é feita por setor, sob demanda, sem carregar a imagem em memória.
- **LBA exposto.** Todo arquivo listado informa setor inicial e tamanho, porque emuladores e ports leem discos por setor.
- **Entrada não confiável.** Imagens malformadas resultam em erro, nunca em pânico. Os parsers serão alvo de fuzzing (`cargo fuzz`).
- **Sem dependências de aplicação.** Nada de parsing de argumentos ou saída de terminal nos crates; isso fica no programa que os usa.
- **Testes sem dados protegidos.** Os testes usam imagens pequenas e sintéticas. Nenhuma imagem de jogo comercial entra no repositório.

## Pendências

- **Fuzzing** dos parsers de descritor e de diretório com `cargo fuzz`.
- **Cabeçalho ELF** em `ps2-disc`.
- **Nome do crate `iso9660`:** já existe um crate com esse nome no crates.io; precisa mudar antes de publicar.
- **Crates existentes:** crates de ISO 9660 já publicados, como `cdfs`, servem como referência e para comparar resultados nos testes.

## Uso previsto

O primeiro consumidor é a `gow2-tool`, a CLI de preparação de um port de PS2 para Windows. Ela usará estes crates para inspecionar a imagem, localizar o ELF de boot e gerar o índice de arquivos e setores.

## Licença

Licenciado sob qualquer uma das opções, à sua escolha:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

Salvo declaração explícita em contrário, qualquer contribuição submetida para inclusão neste trabalho, conforme definido na licença Apache-2.0, será licenciada da forma dupla acima, sem termos ou condições adicionais.
